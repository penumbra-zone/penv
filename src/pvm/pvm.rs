use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use semver::VersionReq;
use target_lexicon::Triple;
use url::Url;

use crate::pvm::release::Release;

use super::{
    cache::cache::Cache, downloader::Downloader, environment::Environment, release::VersionOrLatest,
};

/// The top-level type for the Penumbra Version Manager.
///
/// This type encapsulates application state and exposes higher-level
/// operations.
pub struct Pvm {
    pub cache: Cache,
    pub downloader: Downloader,
    pub environments: Vec<Environment>,
}

impl Pvm {
    /// Create a new instance of the Penumbra Version Manager.
    pub fn new(repository_name: String, home: Utf8PathBuf) -> Result<Self> {
        // TODO: load environments from disk if they exist
        Ok(Self {
            cache: Cache::new(home.clone())?,
            downloader: Downloader::new(repository_name)?,
            environments: Vec::new(),
        })
    }

    pub fn create_environment(
        &mut self,
        environment_alias: String,
        penumbra_version: VersionReq,
        grpc_url: Url,
        repository_name: String,
    ) -> Result<Environment> {
        // Find the best matching version
        let cache = &self.cache;
        let matching_installed_version = match cache.find_best_match(&penumbra_version) {
            Some(installed_version) => installed_version,
            None => {
                // TODO: allow auto-installing here
                return Err(anyhow!(
                    "No installed version found for version requirement {}",
                    penumbra_version
                ));
            }
        };

        let root_dir = cache
            .home
            .join("environments")
            .join(environment_alias.clone());

        let environment = Environment {
            alias: environment_alias.clone(),
            version_requirement: penumbra_version.clone(),
            pinned_version: matching_installed_version.version.clone(),
            grpc_url: grpc_url.clone(),
            root_dir,
        };

        tracing::debug!("created environment: {:?}", environment);

        // Add a reference to the environment to the app
        self.environments.push(environment.clone());

        Ok(environment)
    }

    pub async fn install_release(
        &mut self,
        penumbra_version: VersionOrLatest,
        target_arch: Triple,
    ) -> Result<()> {
        let downloader = &self.downloader;
        let releases = downloader.fetch_releases().await?;

        let mut candidate_releases = Vec::new();
        let latest_version = releases
            .iter()
            .max()
            .ok_or_else(|| anyhow!("No releases found"))?
            .version
            .clone();

        // 3b. find all the versions that satisfy the semver requirement
        'outer: for release in releases {
            if penumbra_version.matches(&release.version, &latest_version) {
                let release_name = release.name.clone();
                tracing::debug!("found candidate release {}", release_name);
                let enriched_release: Release = match release.try_into() {
                    Ok(enriched_release) => enriched_release,
                    Err(e) => {
                        tracing::debug!(
                            "failed to enrich release {}, not making an install candidate: {}",
                            release_name,
                            e
                        );
                        continue;
                    }
                };

                // Typically a release should contain all assets for all architectures,
                // but if it doesn't, this may produce unexpected failures.
                //
                // If the candidate release has no assets for the target architecture, skip it
                let has_arch_asset = enriched_release.assets.iter().any(|asset| {
                    asset.target_arch.is_some()
                        && asset.target_arch.clone().unwrap() == Triple::host()
                });
                if !has_arch_asset {
                    tracing::debug!(
                        "skipping release {} because it has no assets for the target architecture",
                        enriched_release.name
                    );
                    continue 'outer;
                }

                candidate_releases.push(enriched_release);
            }
        }

        if candidate_releases.is_empty() {
            return Err(anyhow!("No matching release found for version requirement"));
        }

        // 4. identify the latest candidate version
        let mut sorted_releases = candidate_releases.clone();
        sorted_releases.sort();

        let latest_release = sorted_releases.last().unwrap();

        // Skip installation if the installed_releases already contains this release
        let cache = &mut self.cache;
        if cache
            .data
            .installed_releases
            .iter()
            .any(|r| r.version == latest_release.version)
        {
            println!("release {} already installed", latest_release.version);
            return Ok(());
        }

        println!(
            "downloading latest matching release: {}",
            latest_release.version
        );
        let installable_release =
            downloader.download_release(latest_release, format!("{}", target_arch))?;
        tracing::debug!("installable release prepared: {:?}", installable_release);

        // 5. attempt to install to the cache
        println!(
            "installing latest matching release: {}",
            latest_release.version
        );
        cache.install_release(&installable_release)?;

        tracing::debug!("persist cache");
        cache.persist()?;

        Ok(())
    }
}
