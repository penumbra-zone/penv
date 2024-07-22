use std::{
    fs::{self, File},
    io::Write as _,
};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::pvm::release::{InstallableRelease, InstalledAsset, InstalledRelease};

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

    pub fn install_release(&mut self, release: &InstallableRelease) -> Result<()> {
        // Identify the paths within the cache to which the release's downloaded assets (currently
        // stored in a temporary directory) should be copied to.
        let version_path = self.get_version_path(release);

        tracing::debug!("creating version path: {}", version_path);
        fs::create_dir_all(&version_path)
            .with_context(|| format!("Failed to create version path directory {}", version_path))?;

        let mut installed_assets = Vec::new();

        // Copy the assets to their target destinations.
        // TODO: reuse code
        for file in release.pcli.iter().flatten() {
            let metadata = fs::metadata(file)?;

            if !metadata.is_file() {
                continue;
            }

            let file_path = version_path.join(file.file_name().expect("expected file name"));

            tracing::debug!("copying: {} to {}", file, file_path);
            fs::copy(file, &file_path)?;

            installed_assets.push(InstalledAsset {
                target_arch: release.target_arch.clone(),
                local_filepath: file_path,
            });
        }

        for file in release.pd.iter().flatten() {
            let metadata = fs::metadata(file)?;

            if !metadata.is_file() {
                continue;
            }

            let file_path = version_path.join(file.file_name().expect("expected file name"));

            tracing::debug!("copying: {} to {}", file, file_path);
            fs::copy(file, &file_path)?;

            installed_assets.push(InstalledAsset {
                target_arch: release.target_arch.clone(),
                local_filepath: file_path,
            });
        }

        for file in release.pclientd.iter().flatten() {
            let metadata = fs::metadata(file)?;

            if !metadata.is_file() {
                continue;
            }

            let file_path = version_path.join(file.file_name().expect("expected file name"));

            tracing::debug!("copying: {} to {}", file, file_path);
            fs::copy(file, &file_path)?;

            installed_assets.push(InstalledAsset {
                target_arch: release.target_arch.clone(),
                local_filepath: file_path,
            });
        }

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
        path.push("bin");

        path
    }

    /// Persist the cache information to disk.
    pub fn persist(&self) -> Result<()> {
        fs::create_dir_all(&self.home)
            .with_context(|| format!("Failed to create home directory {}", self.home))?;

        let toml_cache = toml::to_string(&self.data)?;

        println!("create file: {}", self.config_file_path());
        let mut file = File::create(self.config_file_path())?;
        file.write_all(toml_cache.as_bytes())?;

        Ok(())
    }

    pub fn config_file_path(&self) -> Utf8PathBuf {
        self.home.join("cache.toml")
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
