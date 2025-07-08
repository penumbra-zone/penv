use std::{
    fs::{self, File},
    io::Write as _,
};

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::penv::{
    downloader::Downloader,
    release::{
        Installable as _, InstallableRelease, InstalledRelease, Release, RepoOrVersion,
        RepoOrVersionReq, UsableRelease as _, VersionReqOrLatest,
    },
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

    pub fn delete(&mut self, installed_release: InstalledRelease) -> Result<()> {
        self.data
            .installed_releases
            .retain(|r| r != &installed_release);

        // TODO: calling clone() here defeats the purpose of taking ownership
        <InstalledRelease as Clone>::clone(&installed_release).uninstall()?;

        Ok(())
    }

    /// Find the best matching installed release for a given version/git repo requirement.
    pub fn find_best_match(&self, required: &RepoOrVersionReq) -> Option<&InstalledRelease> {
        // TODO: don't unwrap_or_else here
        let matching_versions = self
            .list_installed(Some(required))
            .unwrap_or_else(|_| vec![]);

        // `InstalledRelease` can't be ordered because there is no meaningful ordering between
        // binary and git repo installations. For a git repo installation, there is currently
        // only a single checkout. Otherwise, the latest binary release is the best match.
        let best_match = match required {
            RepoOrVersionReq::VersionReqOrLatest(ref _r) => {
                matching_versions.iter().max_by_key(|r| match *r {
                    InstalledRelease::Binary(ref r) => r.version.clone(),
                    InstalledRelease::GitCheckout(_) => {
                        unreachable!("no git checkouts should match a versionreq")
                    }
                })
            }
            RepoOrVersionReq::Repo(_) => matching_versions.first(),
        };

        best_match.copied()
    }

    pub(crate) fn install_release(&mut self, release: &InstallableRelease) -> Result<()> {
        // Identify the paths within the cache to which the release's downloaded assets (currently
        // stored in a temporary directory) should be copied to.
        let installed_release_path = self.generate_installed_release_path(release);

        // Copy the assets to their target destinations.
        let installed_release = release.install(installed_release_path)?;

        // Mark the release as installed in the cache
        // TODO: don't reach in data directly...
        self.data.installed_releases.push(installed_release);

        Ok(())
    }

    /// Produces the installation path for a given [`InstallableRelease`]
    // Keeping this here rather than the [`Installable`] trait is preferable because we can have the cache
    // manage its installation directories
    fn generate_installed_release_path(&self, release: &InstallableRelease) -> Utf8PathBuf {
        match release {
            InstallableRelease::Binary(release) => {
                let mut path = self.home.join("versions");
                path.push(release.version().to_string());

                path
            }
            InstallableRelease::GitRepo(metadata) => {
                let mut path = self.home.join("checkouts");

                let target_repo_dir =
                    // TODO: this will only allow a single checkout of a given repo url,
                    // there should maybe be a nonce or index or something to allow multiple checkouts
                    hex::encode(Sha256::digest(metadata.url.to_string().as_bytes()));
                path.push(target_repo_dir);

                path
            }
        }
    }

    pub fn get_installed_release(
        &self,
        repo_or_version: &RepoOrVersion,
    ) -> Option<&InstalledRelease> {
        self.data
            .installed_releases
            .iter()
            .find(|r| r.matches(repo_or_version))
    }

    /// For a binary release with a pinned version, finds the pcli binary for the given version.
    // TODO: maybe move to BinaryRelease and take a Cache ref or something
    pub fn get_pcli_for_version(&self, version: &semver::Version) -> Option<&Utf8PathBuf> {
        let release = self.get_installed_release(&RepoOrVersion::Version(version.clone()))?;

        release.assets().iter().find_map(|a| {
            if a.local_filepath.file_name().unwrap() == "pcli" {
                Some(&a.local_filepath)
            } else {
                None
            }
        })
    }

    pub fn get_pclientd_for_version(&self, version: &semver::Version) -> Option<&Utf8PathBuf> {
        let release = self.get_installed_release(&RepoOrVersion::Version(version.clone()))?;

        release.assets().iter().find_map(|a| {
            if a.local_filepath.file_name().unwrap() == "pclientd" {
                Some(&a.local_filepath)
            } else {
                None
            }
        })
    }

    pub fn get_pd_for_version(&self, version: &semver::Version) -> Option<&Utf8PathBuf> {
        let release = self.get_installed_release(&RepoOrVersion::Version(version.clone()))?;

        release.assets().iter().find_map(|a| {
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

    /// Returns all versions available from the upstream repository,
    /// and whether they're installed, optionally matching a given semver version requirement.
    ///
    /// Only relevant for binary releases right now.
    pub(crate) async fn list_available(
        &self,
        required_version: Option<&RepoOrVersionReq>,
        downloader: &Downloader,
    ) -> Result<Vec<(Release, bool)>> {
        // TODO: fetching of available releases should be cached in the downloader,
        // currently it happens multiple times
        let mut available_releases = downloader.fetch_releases().await?;

        let latest_version = available_releases
            .iter()
            .max()
            .ok_or_else(|| anyhow!("No releases found"))?
            .version
            .clone();

        // Only retain the releases that match the version requirement
        available_releases.retain(|r| {
            if let Some(required_version) = required_version {
                required_version.matches(&r.version, &latest_version)
            } else {
                true
            }
        });

        // Mark each release as installed or not
        let return_releases = available_releases
            .into_iter()
            .map(|r| {
                let installed = self
                    .get_installed_release(&RepoOrVersion::Version(r.version.clone()))
                    .is_some();
                (r, installed)
            })
            .collect();

        Ok(return_releases)
    }

    /// Returns all installed versions, optionally matching a given semver version requirement.
    pub fn list_installed(
        &self,
        required_version: Option<&RepoOrVersionReq>,
    ) -> Result<Vec<&InstalledRelease>> {
        let mut releases = self.data.installed_releases.iter().collect::<Vec<_>>();

        if let Some(required_version) = required_version {
            releases.retain(|r| match (r, required_version) {
                // Binary installed release and version requirement supplied -- matchable
                (
                    InstalledRelease::Binary(r),
                    RepoOrVersionReq::VersionReqOrLatest(version_req),
                ) => match version_req {
                    VersionReqOrLatest::Latest => {
                        unreachable!("can't search installed for 'latest'")
                    }
                    VersionReqOrLatest::VersionReq(version_req) => version_req.matches(&r.version),
                },
                // Checkout release and repo requirement supplied -- matchable
                (InstalledRelease::GitCheckout(checkout), RepoOrVersionReq::Repo(repo)) => {
                    checkout.url == *repo
                }
                // Binary installed release and repo requirement supplied -- not matchable
                (InstalledRelease::Binary(_), RepoOrVersionReq::Repo(_)) => false,
                // Checkout release and version requirement supplied -- not matchable
                (InstalledRelease::GitCheckout(_), RepoOrVersionReq::VersionReqOrLatest(_)) => {
                    false
                }
            })
        }

        Ok(releases)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use semver::Version;
    use target_lexicon::Triple;

    use crate::penv::release::{binary::InstalledBinaryRelease, InstalledAsset};

    use super::*;

    #[test]
    fn deserialize_cache() {
        let cache_data = CacheData {
            installed_releases: vec![InstalledRelease::Binary(InstalledBinaryRelease {
                version: Version::parse("1.0.0").unwrap(),
                body: Some("Release notes for version 1.0.0".to_string()),
                assets: vec![InstalledAsset {
                    target_arch: Triple::from_str("x86_64-unknown-linux-gnu").unwrap(),
                    local_filepath: Utf8PathBuf::from("/tmp/fake"),
                }],
                name: "Release 1.0.0".to_string(),
                root_dir: Utf8PathBuf::from("/tmp/fake"),
            })],
        };

        // Serialize to TOML string
        eprintln!("{}", toml::to_string(&cache_data).unwrap());

        // Example TOML string for deserialization
        let toml_str = r#"
        [[installed_releases]]
        type = "Binary"

        [installed_releases.args]
        version = "1.0.0"
        body = "Release notes for version 1.0.0"
        name = "Release 1.0.0"
        root_dir = "/tmp/fake"

        [[installed_releases.args.assets]]
        target_arch = "x86_64-unknown-linux-gnu"
        local_filepath = "/tmp/fake"
        "#;

        // Deserialize from TOML string
        toml::from_str::<CacheData>(toml_str).unwrap();
    }
}
