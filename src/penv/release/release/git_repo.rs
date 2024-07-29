use std::fmt::{self, Display};

use anyhow::{Context as _, Result};
use camino::Utf8PathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::penv::{downloader::git::clone_repo, release::InstalledAsset};

use super::{Installable, InstalledRelease, UsableRelease};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMetadata {
    // TODO: is there a type from gix that we can use here?
    pub name: String,
    pub url: String,
    pub checkout_dir: Utf8PathBuf,
}

impl Display for RepoMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.url)
    }
}

impl Installable for RepoMetadata {
    fn version(&self) -> Option<&Version> {
        None
    }

    fn install(&self, install_path: Utf8PathBuf) -> Result<InstalledRelease> {
        // Clone the repository into the install path
        // TODO: is there any reason to do this instead of just cloning the release on-demand
        // into the environment's checkout dir? we copy it later eventually
        clone_repo(&self.url, &install_path.to_string()).context("error cloning repository")?;

        Ok(InstalledRelease::GitCheckout(CheckoutMetadata {
            name: self.name.clone(),
            url: self.url.clone(),
            install_path,
        }))
    }
}

/// Metadata defining a git checkou
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckoutMetadata {
    // TODO: is there a type from gix that we can use here?
    pub name: String,
    pub url: String,
    // TODO: the checkout has two parts, the code and the binary aliases
    // the environment struct should maintain these paths, or they should be
    // symlinked
    pub install_path: Utf8PathBuf,
}

impl Display for CheckoutMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.url)
    }
}

impl UsableRelease for CheckoutMetadata {
    /// There are no assets associated with a git checkout.
    fn assets(&self) -> &[InstalledAsset] {
        &[]
    }

    fn uninstall(self) -> Result<()> {
        let checkout_dir = &self.install_path;
        if checkout_dir.exists() {
            tracing::debug!("deleting checkout directory: {}", checkout_dir);
            std::fs::remove_dir_all(&checkout_dir).context("error removing checkout directory")?;
        }

        Ok(())
    }
}
