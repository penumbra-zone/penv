use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::value_parser;
use target_lexicon::Triple;

use crate::pvm::{
    cache::cache::Cache,
    downloader::Downloader,
    release::{Release, VersionOrLatest},
};

// TODO: the cometbft version being used must be matched to the penumbra versions,
// so it might be desirable to add support for managing cometbft installations here as well.

#[derive(Debug, clap::Parser)]
pub struct InstallCmd {
    /// The GitHub repository to fetch releases from.
    ///
    /// Defaults to "penumbra-zone/penumbra"
    #[clap(long, default_value = "penumbra-zone/penumbra")]
    repository_name: String,
    /// The string "latest", or the version of the Penumbra software suite to install.
    ///
    /// Specified as a semver version requirement, i.e. "0.79" will install the latest 0.79.x release.
    #[clap(value_parser = value_parser!(VersionOrLatest))]
    penumbra_version: VersionOrLatest,
}

impl InstallCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let repository_name = &self.repository_name;

        let downloader = Downloader::new(repository_name.clone())?;
        println!("installing {}", self.penumbra_version);
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
            if self
                .penumbra_version
                .matches(&release.version, &latest_version)
            {
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
        let cache = Cache::new(home.clone())?;
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
            downloader.download_release(latest_release, format!("{}", Triple::host()))?;
        tracing::debug!("installable release prepared: {:?}", installable_release);

        // 5. attempt to install to the cache
        println!(
            "installing latest matching release: {}",
            latest_release.version
        );
        let mut cache = Cache::new(home)?;
        cache.install_release(&installable_release)?;

        tracing::debug!("persist cache");
        cache.persist()?;

        Ok(())
    }
}
