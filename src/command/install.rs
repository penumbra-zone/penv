use anyhow::Result;
use camino::Utf8PathBuf;
use clap::value_parser;
use target_lexicon::Triple;

use crate::pvm::{release::VersionOrLatest, Pvm};

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

        println!("installing {}", self.penumbra_version);
        let mut pvm = Pvm::new_from_repository(repository_name.clone(), home.clone())?;
        pvm.install_release(self.penumbra_version.clone(), Triple::host())
            .await?;

        Ok(())
    }
}
