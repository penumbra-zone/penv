use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_LENGTH, USER_AGENT};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Write};
use tar::Archive;
use tempfile::{tempdir, TempDir};

use crate::pvm::release::{InstallableRelease, RawRelease, Release};

/// Manages downloading files to a target directory and displaying progress.
pub(crate) struct Downloader {
    client: Client,
    temp_dir: TempDir,
    repository_name: String,
}

impl Downloader {
    pub fn new(repository_name: String) -> Result<Self> {
        // Create a temporary directory
        let temp_dir = tempdir()?;

        Ok(Self {
            client: Client::new(),
            temp_dir,
            repository_name,
        })
    }

    pub fn get_contents(&self, url: &str) -> Result<Vec<u8>> {
        println!("downloading shasum from {}", url);

        // Send the GET request and get the response
        let response = self.client.get(url).send()?.error_for_status()?;

        // Read the response bytes into a Vec<u8>
        let content = response.bytes()?.to_vec();

        Ok(content)
    }

    pub fn download(
        &self,
        url: &str,
        expected_shasum: Option<Vec<u8>>,
        display_progress: bool,
    ) -> Result<Vec<Utf8PathBuf>> {
        println!("downloading archive from {}", url);

        // Get the name of the file from the URL
        let file_name = url
            .rsplit('/')
            .next()
            .ok_or_else(|| anyhow!("Failed to get file name from URL"))?;

        // Send the GET request and get the response
        let response = self.client.get(url).send()?.error_for_status()?;

        // Get the content length for the progress bar
        let total_size = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|ct_len| ct_len.to_str().ok())
            .and_then(|ct_len| ct_len.parse().ok())
            .unwrap_or(0);

        let mut pb = if display_progress {
            let pb = ProgressBar::new(total_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };

        // Create a temporary file for the downloaded archive
        // TODO: we don't want to download to the final location in case the shasum mismatches,
        // however we could return an in-memory buffer directly instead of a tempfile to save
        // some filesystem operations
        let temp_file_path = self.temp_dir.path().join(file_name);
        let mut temp_file = File::create(&temp_file_path)?;

        // Create a buffer for reading the response content
        let mut content = io::Cursor::new(response.bytes()?);

        // Create a Sha256 hasher
        let mut hasher = Sha256::new();

        // Write the response content to the temporary file with progress bar
        let mut buffer = [0; 1024];
        loop {
            let bytes_read = content.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            temp_file.write_all(&buffer[..bytes_read])?;
            hasher.update(&buffer[..bytes_read]);
            pb.as_mut().map(|pb| pb.inc(bytes_read as u64));
        }
        pb.map(|pb| pb.finish_with_message("Download complete"));

        // Verify the SHA-256 checksum, if available
        if expected_shasum.is_some() {
            let expected_shasum = expected_shasum.clone().unwrap();
            let calculated_hash = hasher.finalize().to_vec();

            if calculated_hash != expected_shasum {
                return Err(anyhow!(
                    "SHA-256 checksum mismatch: expected {}, got {}",
                    hex::encode(expected_shasum),
                    hex::encode(calculated_hash)
                ));
            }
        } else {
            tracing::debug!("skipping sha256sum verification, none available");
        }

        // Reopen the file and create a decompressor
        let temp_file = File::open(&temp_file_path)?;
        let decompressor = GzDecoder::new(temp_file);

        // Create a tar archive from the decompressed content
        let mut archive = Archive::new(decompressor);
        // Collect the list of extracted files
        let mut extracted_files = Vec::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_path_buf();
            let full_path = self.temp_dir.path().join(&path);

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write the entry to disk
            entry.unpack(&full_path)?;

            extracted_files.push(Utf8PathBuf::try_from(full_path)?);
        }

        Ok(extracted_files)
    }

    pub async fn fetch_releases(&self) -> Result<Vec<Release>> {
        println!(
            "fetching available releases from https://api.github.com/repos/{}/releases",
            self.repository_name
        );

        let repository_name = self.repository_name.clone();

        // 1. fetch the repository JSON:
        // Set up the headers
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("curl/7.68.0"));

        let response = self
            .client
            .get(format!(
                "https://api.github.com/repos/{repository_name}/releases"
            ))
            .headers(headers)
            .send()?;

        // let mut candidate_releases = Vec::new();
        // Check if the response is successful
        if response.status().is_success() {
            // Parse the JSON response into a Rust struct
            let releases: Vec<RawRelease> = response.json()?;

            // 3a. Enrich all the releases with proper domain types
            let enriched_releases: Vec<Release> = releases
                .iter()
                .map(|r| r.try_into())
                .collect::<Result<_>>()?;

            Ok(enriched_releases)
        } else {
            Err(anyhow!("Failed to fetch releases"))
        }
    }

    /// Turns a Release into an InstallableRelease by matching the assets to the
    /// currently active architecture.
    pub fn download_release(
        &self,
        release: &Release,
        target_arch: String,
    ) -> Result<InstallableRelease> {
        let mut pcli = None;
        let mut pclientd = None;
        let mut pd = None;

        // Match the archive asset files to their respective shasums.
        let mut pcli_shasum = None;
        let mut pclientd_shasum = None;
        let mut pd_shasum = None;

        // First download shasums
        for asset in &release.assets {
            if !asset.browser_download_url.contains(&target_arch) {
                continue;
            }

            if asset.browser_download_url.contains("pcli-") {
                if asset.browser_download_url.ends_with(".sha256") {
                    // download the shasum
                    let shasum_str = self.get_contents(&asset.browser_download_url)?;
                    let shasum = hex::decode(&shasum_str[..64])?;
                    pcli_shasum = Some(shasum);
                    continue;
                }
            } else if asset.browser_download_url.contains("pclientd-") {
                if asset.browser_download_url.ends_with(".sha256") {
                    // download the shasum
                    let shasum_str = self.get_contents(&asset.browser_download_url)?;
                    let shasum = hex::decode(&shasum_str[..64])?;
                    pclientd_shasum = Some(shasum);
                    continue;
                }
            } else if asset.browser_download_url.contains("pd-") {
                if asset.browser_download_url.ends_with(".sha256") {
                    // download the shasum
                    let shasum_str = self.get_contents(&asset.browser_download_url)?;
                    let shasum = hex::decode(&shasum_str[..64])?;
                    pd_shasum = Some(shasum);
                    continue;
                }
            }
        }

        // Then download archives
        for asset in &release.assets {
            if !asset.browser_download_url.contains(&target_arch) {
                continue;
            }

            if asset.browser_download_url.contains("pcli-") {
                if asset.browser_download_url.ends_with(".tar.gz") {
                    let downloaded_asset =
                        self.download(&asset.browser_download_url, pcli_shasum.clone(), true)?;

                    pcli = Some(downloaded_asset);
                    continue;
                }
            } else if asset.browser_download_url.contains("pclientd-") {
                if asset.browser_download_url.ends_with(".tar.gz") {
                    let downloaded_asset =
                        self.download(&asset.browser_download_url, pclientd_shasum.clone(), true)?;

                    pclientd = Some(downloaded_asset);
                    continue;
                }
            } else if asset.browser_download_url.contains("pd-") {
                if asset.browser_download_url.ends_with(".tar.gz") {
                    let downloaded_asset =
                        self.download(&asset.browser_download_url, pd_shasum.clone(), true)?;

                    pd = Some(downloaded_asset);
                    continue;
                }
            }
        }

        Ok(InstallableRelease {
            pcli,
            pclientd,
            pd,
            release: release.clone(),
            target_arch: target_arch.parse()?,
        })
    }
}
