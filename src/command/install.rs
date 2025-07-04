use anyhow::Result;
use camino::Utf8PathBuf;
use clap::value_parser;
use target_lexicon::Triple;

use crate::penv::{release::RepoOrVersionReq, Penv};

// TODO: the cometbft version being used must be matched to the penumbra versions,
// so it might be desirable to add support for managing cometbft installations here as well.

#[derive(Debug, clap::Parser)]
pub struct InstallCmd {
    /// The GitHub repository to fetch releases from.
    ///
    /// Defaults to "penumbra-zone/penumbra"
    #[clap(long, default_value = "penumbra-zone/penumbra")]
    repository_name: String,
    /// The version of the Penumbra software suite to install.
    ///
    /// This can install either from a git repo (locally or remotely) or from a version requirement.
    ///
    /// Git repo examples:
    ///
    /// - https://github.com/penumbra-zone/penumbra.git
    /// - git@github.com:penumbra-zone/penumbra.git
    /// - /Users/user/repos/penumbra
    ///
    /// Valid version requirements:
    ///
    /// - The string "latest", or the version of the Penumbra software suite to install.
    ///
    /// Version requirements are specified as a semver version requirement, i.e. "0.79" will install the latest 0.79.x release.
    #[clap(value_parser = value_parser!(RepoOrVersionReq))]
    penumbra_version: RepoOrVersionReq,
}

impl InstallCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let repository_name = &self.repository_name;

        println!("installing {}", self.penumbra_version);
        let mut penv = Penv::new_from_repository(repository_name.clone(), home.clone())?;
        penv.install_release(self.penumbra_version.clone(), Triple::host())
            .await?;

        Ok(())
    }
}
