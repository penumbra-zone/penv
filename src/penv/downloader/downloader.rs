use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use flate2::read::GzDecoder;
use futures::stream::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tar::Archive;
use tempfile::{tempdir, TempDir};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task;

use crate::penv::release::{InstallableBinaryRelease, InstallableRelease, RawRelease, Release};

/// Manages downloading files to a target directory and displaying progress.
#[derive(Debug, Clone)]
pub(crate) struct Downloader {
    client: Client,
    temp_dir: Arc<TempDir>,
    repository_name: String,
}

impl Downloader {
    pub fn new(repository_name: String) -> Result<Self> {
        // Create a temporary directory
        let temp_dir = Arc::new(tempdir()?);

        Ok(Self {
            client: Client::new(),
            temp_dir,
            repository_name,
        })
    }

    pub async fn get_contents(&self, url: &str) -> Result<Vec<u8>> {
        println!("downloading shasum from {}", url);

        // Send the GET request and get the response
        // let response = self.client.get(url).send()?.error_for_status()?;
        let response = self.client.get(url).send().await?.error_for_status()?;

        // Read the response bytes into a Vec<u8>
        let content = response.bytes().await?.to_vec();

        Ok(content)
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
            .send()
            .await?
            .error_for_status()?;

        // let mut candidate_releases = Vec::new();
        // Check if the response is successful
        if response.status().is_success() {
            // Parse the JSON response into a Rust struct
            let releases: Vec<RawRelease> = response.json().await?;

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

    async fn download_file(
        &self,
        url: String,
        file_path: Utf8PathBuf,
        progress_bar: ProgressBar,
        // TODO: there should be a cli flag to bypass shasum
        // verification, as it's best to force the user to bypass it
        // rather than ever doing so implicitly
        expected_shasum: Option<Vec<u8>>,
    ) -> Result<(String, Vec<Utf8PathBuf>)> {
        println!("downloading archive from {}", url);

        // Get the name of the file from the URL
        let file_name = url
            .rsplit('/')
            .next()
            .ok_or_else(|| anyhow!("Failed to get file name from URL"))?;

        let response = self
            .client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;

        let total_size = response.content_length().unwrap_or(0);
        progress_bar.set_length(total_size);

        let mut file = File::create(file_path.clone()).await?;
        let mut stream = response.bytes_stream();

        // Create a Sha256 hasher
        let mut hasher = Sha256::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            progress_bar.inc(chunk.len() as u64);
        }

        progress_bar.finish_with_message("Downloaded");

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
        let temp_file = File::open(&file_path).await?;
        let decompressor = GzDecoder::new(
            temp_file
                .try_into_std()
                .map_err(|_| anyhow!("unable to convert tokio::fs::File to std::fs::File"))?,
        );

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

        Ok((file_name.to_string(), extracted_files))
    }

    /// Turns a Release into an InstallableRelease by matching the assets to the
    /// currently active architecture.
    pub async fn download_release(
        &self,
        release: &Release,
        target_arch: String,
    ) -> Result<InstallableRelease> {
        // TODO: make downloads take place in multiple tasks simultaneously
        let mut pcli = None;
        let mut pclientd = None;
        let mut pd = None;

        // Match the archive asset files to their respective shasums.
        let mut pcli_shasum = None;
        let mut pclientd_shasum = None;
        let mut pd_shasum = None;

        // First download shasums
        let mut shasum_urls = Vec::new();
        for asset in &release.assets {
            if !asset.browser_download_url.contains(&target_arch) {
                continue;
            }

            if asset.browser_download_url.contains("pcli-") {
                if asset.browser_download_url.ends_with(".sha256") {
                    shasum_urls.push(asset.browser_download_url.clone());
                    // // download the shasum
                    // let shasum_str = self.get_contents(&asset.browser_download_url).await?;
                    // let shasum = hex::decode(&shasum_str[..64])?;
                    // pcli_shasum = Some(shasum);
                    continue;
                }
            } else if asset.browser_download_url.contains("pclientd-") {
                if asset.browser_download_url.ends_with(".sha256") {
                    shasum_urls.push(asset.browser_download_url.clone());
                    // download the shasum
                    // let shasum_str = self.get_contents(&asset.browser_download_url).await?;
                    // let shasum = hex::decode(&shasum_str[..64])?;
                    // pclientd_shasum = Some(shasum);
                    continue;
                }
            } else if asset.browser_download_url.contains("pd-")
                && asset.browser_download_url.ends_with(".sha256")
            {
                shasum_urls.push(asset.browser_download_url.clone());
                // // download the shasum
                // let shasum_str = self.get_contents(&asset.browser_download_url).await?;
                // let shasum = hex::decode(&shasum_str[..64])?;
                // pd_shasum = Some(shasum);
                continue;
            }
        }

        let multi_progress = MultiProgress::new();
        let mut handles = Vec::new();
        let arc_self = Arc::new(self.clone());

        for shasum_url in shasum_urls {
            let progress_bar = multi_progress.add(ProgressBar::new(0));
            progress_bar.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("#>-"));

            // Create a temporary file for the downloaded archive
            // TODO: we don't want to download to the final location in case the shasum mismatches,
            // however we could return an in-memory buffer directly instead of a tempfile to save
            // some filesystem operations
            let file_name = shasum_url
                .rsplit('/')
                .next()
                .ok_or_else(|| anyhow!("Failed to get file name from URL"))?
                .to_string();
            // let temp_file_path = Utf8PathBuf::from_path_buf(self.temp_dir.path().join(file_name))
            //     .map_err(|_| anyhow!("Failed to create temp file path"))?;

            // let shasum_str = self.get_contents(&asset.browser_download_url).await?;
            // let shasum = hex::decode(&shasum_str[..64])?;
            // pd_shasum = Some(shasum);
            let arc_self = arc_self.clone();
            let handle = tokio::spawn(async move {
                let contents = arc_self
                    .get_contents(&shasum_url)
                    .await
                    .expect("failed to download shasum");

                task::yield_now().await;

                (file_name.clone(), contents)
            });
            handles.push(handle);
        }

        // Drive the multi-progress bar in a separate task
        let mp_thread = std::thread::spawn(move || multi_progress);

        // Await the shasums and set them
        for handle in handles {
            let (file_name, contents) = handle.await?;
            if file_name.starts_with("pd-") && file_name.ends_with(".sha256") {
                let shasum = hex::decode(&contents[..64])?;
                pd_shasum = Some(shasum);
            } else if file_name.starts_with("pcli-") && file_name.ends_with(".sha256") {
                let shasum = hex::decode(&contents[..64])?;
                pcli_shasum = Some(shasum);
            } else if file_name.starts_with("pclientd-") && file_name.ends_with(".sha256") {
                let shasum = hex::decode(&contents[..64])?;
                pclientd_shasum = Some(shasum);
            }
        }

        mp_thread
            .join()
            .expect("failed to join the multiprogress bar thread");

        // Then download archives
        let multi_progress = MultiProgress::new();
        let mut handles = Vec::new();
        for asset in &release.assets {
            if !asset.browser_download_url.contains(&target_arch) {
                continue;
            }

            let progress_bar = multi_progress.add(ProgressBar::new(0));
            progress_bar.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("#>-"));

            if asset.browser_download_url.contains("pcli-") {
                if asset.browser_download_url.ends_with(".tar.gz") {
                    let arc_self = arc_self.clone();
                    let pcli_shasum = pcli_shasum.clone();
                    let url = asset.browser_download_url.clone();
                    let file_name = url
                        .rsplit('/')
                        .next()
                        .ok_or_else(|| anyhow!("Failed to get file name from URL"))?
                        .to_string();
                    let temp_file_path =
                        Utf8PathBuf::from_path_buf(self.temp_dir.path().join(&file_name))
                            .map_err(|_| anyhow!("Failed to create temp file path"))?;
                    let handle = tokio::spawn(async move {
                        let (file_name, downloaded_files) = arc_self
                            .download_file(url, temp_file_path.clone(), progress_bar, pcli_shasum)
                            .await
                            .expect("failed to download archive");

                        task::yield_now().await;

                        (file_name, downloaded_files)
                    });
                    handles.push(handle);

                    continue;
                }
            } else if asset.browser_download_url.contains("pclientd-") {
                if asset.browser_download_url.ends_with(".tar.gz") {
                    let arc_self = arc_self.clone();
                    let pclientd_shasum = pclientd_shasum.clone();
                    let url = asset.browser_download_url.clone();
                    let file_name = url
                        .rsplit('/')
                        .next()
                        .ok_or_else(|| anyhow!("Failed to get file name from URL"))?
                        .to_string();
                    let temp_file_path =
                        Utf8PathBuf::from_path_buf(self.temp_dir.path().join(&file_name))
                            .map_err(|_| anyhow!("Failed to create temp file path"))?;
                    let handle = tokio::spawn(async move {
                        let (file_name, downloaded_files) = arc_self
                            .download_file(
                                url,
                                temp_file_path.clone(),
                                progress_bar,
                                pclientd_shasum,
                            )
                            .await
                            .expect("failed to download archive");

                        task::yield_now().await;

                        (file_name, downloaded_files)
                    });
                    handles.push(handle);

                    continue;
                }
            } else if asset.browser_download_url.contains("pd-")
                && asset.browser_download_url.ends_with(".tar.gz")
            {
                let arc_self = arc_self.clone();
                let pd_shasum = pd_shasum.clone();
                let url = asset.browser_download_url.clone();
                let file_name = url
                    .rsplit('/')
                    .next()
                    .ok_or_else(|| anyhow!("Failed to get file name from URL"))?
                    .to_string();
                let temp_file_path =
                    Utf8PathBuf::from_path_buf(self.temp_dir.path().join(&file_name))
                        .map_err(|_| anyhow!("Failed to create temp file path"))?;
                let handle = tokio::spawn(async move {
                    let (file_name, downloaded_files) = arc_self
                        .download_file(url, temp_file_path.clone(), progress_bar, pd_shasum)
                        .await
                        .expect("failed to download archive");

                    task::yield_now().await;

                    (file_name, downloaded_files)
                });
                handles.push(handle);

                continue;
            }
        }

        // Drive the multi-progress bar in a separate task
        let mp_thread = std::thread::spawn(move || multi_progress);

        for handle in handles {
            let (file_name, file_path) = handle.await?;
            if file_name.starts_with("pd-") {
                pd = Some(
                    file_path
                        .iter()
                        .find(|p| p.ends_with("pd"))
                        .unwrap()
                        .clone(),
                );
            } else if file_name.starts_with("pcli-") {
                pcli = Some(
                    file_path
                        .iter()
                        .find(|p| p.ends_with("pcli"))
                        .unwrap()
                        .clone(),
                );
            } else if file_name.starts_with("pclientd-") {
                pclientd = Some(
                    file_path
                        .iter()
                        .find(|p| p.ends_with("pclientd"))
                        .unwrap()
                        .clone(),
                );
            }
        }

        mp_thread.join().unwrap();

        Ok(InstallableRelease::Binary(InstallableBinaryRelease {
            pcli,
            pclientd,
            pd,
            release: release.clone(),
            target_arch: target_arch.parse()?,
        }))
    }
}
