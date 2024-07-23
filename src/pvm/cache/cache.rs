use std::{
    fs::{self, File},
    io::Write as _,
};

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::pvm::{
    downloader::Downloader,
    release::{InstallableRelease, InstalledAsset, InstalledRelease, Release},
};

/// The Cache is responsible for maintaining a directory of all installed software versions.
#[derive(Debug)]
pub struct Cache {
    pub home: Utf8PathBuf,
    pub data: CacheData,
}

/// Data to persist regarding a cache instance.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheData {
    pub installed_releases: Vec<InstalledRelease>,
}

impl Cache {
    pub fn new(home: Utf8PathBuf) -> Result<Self> {
        // read config file to fetch installed releases
        let config_path = home.join("cache.toml");
        let metadata = fs::metadata(&config_path);

        let data = if metadata.is_err() || !metadata.unwrap().is_file() {
            CacheData {
                installed_releases: Vec::new(),
            }
        } else {
            let config_contents = fs::read_to_string(config_path)?;
            toml::from_str(&config_contents)?
        };

        Ok(Self { home, data })
    }

    pub fn delete(&mut self, version: &Version) -> Result<()> {
        let installed_version = self
            .get_installed_release(version)
            .ok_or_else(|| anyhow::anyhow!("No installed version found for version {}", version))?;

        let installed_version_dir = &installed_version.root_dir;
        if installed_version_dir.exists() {
            tracing::debug!("deleting version directory: {}", installed_version_dir);
            std::fs::remove_dir_all(&installed_version_dir)
                .context("error removing version directory")?;
        }

        self.data
            .installed_releases
            .retain(|r| r.version != *version);

        Ok(())
    }

    pub fn find_best_match(
        &self,
        required_version: &semver::VersionReq,
    ) -> Option<&InstalledRelease> {
        let matching_versions = self
            .list(Some(required_version))
            .or_else(|_| Ok::<Vec<&InstalledRelease>, anyhow::Error>(vec![]))
            .unwrap();

        matching_versions.iter().max_by_key(|r| &r.version).copied()
    }

    pub(crate) fn install_release(&mut self, release: &InstallableRelease) -> Result<()> {
        // Identify the paths within the cache to which the release's downloaded assets (currently
        // stored in a temporary directory) should be copied to.
        let version_path = self.get_version_path(release);
        let version_bin_path = version_path.join("bin");

        tracing::debug!("creating version bin path: {}", version_path);
        fs::create_dir_all(&version_bin_path).with_context(|| {
            format!(
                "Failed to create version bin path directory {}",
                version_path
            )
        })?;

        let mut installed_assets = Vec::new();

        // Copy the assets to their target destinations.
        // TODO: reuse code
        let file = release.pcli.as_ref().expect("expected pcli file");
        let metadata = fs::metadata(file)?;

        if !metadata.is_file() {
            return Err(anyhow!("missing pcli"));
        }

        let file_path = version_bin_path.join(file.file_name().expect("expected file name"));

        tracing::debug!("copying: {} to {}", file, file_path);
        fs::copy(file, &file_path)?;

        installed_assets.push(InstalledAsset {
            target_arch: release.target_arch.clone(),
            local_filepath: file_path,
        });

        let file = release.pd.as_ref().expect("expected pd file");
        let metadata = fs::metadata(file)?;

        if !metadata.is_file() {
            return Err(anyhow!("missing pd"));
        }

        let file_path = version_bin_path.join(file.file_name().expect("expected file name"));

        tracing::debug!("copying: {} to {}", file, file_path);
        fs::copy(file, &file_path)?;

        installed_assets.push(InstalledAsset {
            target_arch: release.target_arch.clone(),
            local_filepath: file_path,
        });

        let file = release.pclientd.as_ref().expect("expected pclientd file");
        let metadata = fs::metadata(file)?;

        if !metadata.is_file() {
            return Err(anyhow!("missing pclientd"));
        }

        let file_path = version_bin_path.join(file.file_name().expect("expected file name"));

        tracing::debug!("copying: {} to {}", file, file_path);
        fs::copy(file, &file_path)?;

        installed_assets.push(InstalledAsset {
            target_arch: release.target_arch.clone(),
            local_filepath: file_path,
        });

        // Mark the release as installed in the cache
        // TODO: don't reach in data directly...
        let installed_release = InstalledRelease {
            version: release.version().clone(),
            body: release.release.body.clone(),
            assets: installed_assets,
            name: release.release.name.clone(),
            root_dir: version_path,
        };
        self.data.installed_releases.push(installed_release);

        Ok(())
    }

    fn get_version_path(&self, release: &InstallableRelease) -> Utf8PathBuf {
        let mut path = self.home.join("versions");
        path.push(&release.version().to_string());

        path
    }

    pub fn get_installed_release(&self, version: &semver::Version) -> Option<&InstalledRelease> {
        self.data
            .installed_releases
            .iter()
            .find(|r| &r.version == version)
    }

    pub fn get_pcli_for_version(&self, version: &semver::Version) -> Option<&Utf8PathBuf> {
        let release = self.get_installed_release(version)?;

        release.assets.iter().find_map(|a| {
            if a.local_filepath.file_name().unwrap() == "pcli" {
                Some(&a.local_filepath)
            } else {
                None
            }
        })
    }

    pub fn get_pclientd_for_version(&self, version: &semver::Version) -> Option<&Utf8PathBuf> {
        let release = self.get_installed_release(version)?;

        release.assets.iter().find_map(|a| {
            if a.local_filepath.file_name().unwrap() == "pclientd" {
                Some(&a.local_filepath)
            } else {
                None
            }
        })
    }

    pub fn get_pd_for_version(&self, version: &semver::Version) -> Option<&Utf8PathBuf> {
        let release = self.get_installed_release(version)?;

        release.assets.iter().find_map(|a| {
            if a.local_filepath.file_name().unwrap() == "pd" {
                Some(&a.local_filepath)
            } else {
                None
            }
        })
    }

    /// Persist the cache information to disk.
    pub fn persist(&self) -> Result<()> {
        fs::create_dir_all(&self.home)
            .with_context(|| format!("Failed to create home directory {}", self.home))?;

        let toml_cache = toml::to_string(&self.data)?;

        tracing::debug!(config_file_path=?self.config_file_path(),"create file");
        let mut file = File::create(self.config_file_path())?;
        file.write_all(toml_cache.as_bytes())?;

        Ok(())
    }

    pub fn config_file_path(&self) -> Utf8PathBuf {
        self.home.join("cache.toml")
    }

    /// Returns all available versions and whether they're installed, optionally matching a given semver version requirement.
    pub(crate) async fn list_available(
        &self,
        required_version: Option<&semver::VersionReq>,
        downloader: &Downloader,
    ) -> Result<Vec<(Release, bool)>> {
        let mut available_releases = downloader.fetch_releases().await?;

        // Only retain the releases that match the version requirement
        available_releases = available_releases
            .into_iter()
            .filter(|r| {
                if let Some(required_version) = required_version {
                    required_version.matches(&r.version)
                } else {
                    true
                }
            })
            .collect();

        // Mark each release as installed or not
        let return_releases = available_releases
            .into_iter()
            .map(|r| {
                let installed = self.get_installed_release(&r.version).is_some();
                (r, installed)
            })
            .collect();

        Ok(return_releases)
    }

    /// Returns all installed versions, optionally matching a given semver version requirement.
    pub fn list(
        &self,
        required_version: Option<&semver::VersionReq>,
    ) -> Result<Vec<&InstalledRelease>> {
        let mut releases = self.data.installed_releases.iter().collect::<Vec<_>>();

        if let Some(required_version) = required_version {
            releases.retain(|r| required_version.matches(&r.version));
        }

        Ok(releases)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use semver::Version;
    use target_lexicon::Triple;

    use crate::pvm::release::InstalledAsset;

    use super::*;

    #[test]
    fn deserialize_cache() {
        let cache_data = CacheData {
            installed_releases: vec![InstalledRelease {
                version: Version::parse("1.0.0").unwrap(),
                body: Some("Release notes for version 1.0.0".to_string()),
                assets: vec![InstalledAsset {
                    target_arch: Triple::from_str("x86_64-unknown-linux-gnu").unwrap(),
                    local_filepath: Utf8PathBuf::from("/tmp/fake"),
                }],
                name: "Release 1.0.0".to_string(),
                root_dir: Utf8PathBuf::from("/tmp/fake"),
            }],
        };

        // Serialize to TOML string
        eprintln!("{}", toml::to_string(&cache_data).unwrap());

        // Example TOML string for deserialization
        let toml_str = r#"
            [[installed_releases]]
            version = "1.0.0"
            body = "Release notes for version 1.0.0"
            name = "Release 1.0.0"
            root_dir = "/tmp/fake"

            [[installed_releases.assets]]
            target_arch = "x86_64-unknown-linux-gnu"
            local_filepath = "/tmp/fake"
        "#;

        // Deserialize from TOML string
        toml::from_str::<CacheData>(toml_str).unwrap();
    }
}
