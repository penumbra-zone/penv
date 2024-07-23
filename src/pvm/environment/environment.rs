use anyhow::{anyhow, Context as _, Result};
use std::os::unix::fs::symlink as unix_symlink;
#[cfg(target_os = "windows")]
use std::os::windows::fs::symlink_file as windows_symlink_file;
use std::{
    fs,
    ops::{Deref, DerefMut},
    os::unix::fs::PermissionsExt as _,
    process::Command,
};

use camino::Utf8PathBuf;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::pvm::cache::cache::Cache;
use crate::pvm::environment::Binary as _;

use super::PcliBinary;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Environment {
    pub alias: String,
    pub grpc_url: Url,
    pub version_requirement: VersionReq,
    // TODO: implement a way to update the pinned_version
    // to the latest matching the version_requirement
    pub pinned_version: Version,
    pub root_dir: Utf8PathBuf,
    // TODO: include whether there should be a pd config generated as well
}

impl Environment {
    /// Initializes an environment on disk, by creating the necessary
    /// pd/pclientd/pcli configurations and symlinks to the
    /// pinned version of the software stack.
    pub fn initialize(&self, cache: &Cache) -> Result<()> {
        // Create the directory structure for the environment
        let bin_dir = self.root_dir.join("bin");
        tracing::debug!("creating bin_dir at {}", bin_dir);
        fs::create_dir_all(&bin_dir)
            .with_context(|| format!("Failed to create bin directory {}", bin_dir))?;

        // Create symlinks for the pinned version of the software stack
        self.create_symlinks(cache)?;

        // Initialize pcli configuration
        // Since the initialization is version-dependent, it is necessary
        // to shell out to the installed binary to perform the initialization.
        let pcli_binary = self.get_pcli_binary();
        pcli_binary.initialize()?;

        Ok(())
    }

    // TODO: seems like the various binaries should be made
    // into instances of some kind of trait
    pub fn pcli_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pcli")
    }

    pub fn get_pcli_binary(&self) -> PcliBinary {
        PcliBinary {
            pcli_data_dir: self.pcli_data_dir(),
            root_dir: self.root_dir.clone(),
            grpc_url: self.grpc_url.clone(),
        }
    }

    pub fn pclientd_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pclientd")
    }

    pub fn pd_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pd")
    }

    pub fn pcli_data_dir(&self) -> Utf8PathBuf {
        self.root_dir.join("pcli")
    }

    fn create_symlinks(&self, cache: &Cache) -> Result<()> {
        create_symlink(
            cache
                .get_pcli_for_version(&self.pinned_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pcli version found for version {}",
                        self.pinned_version
                    )
                })?,
            &self.pcli_path(),
        )
        .context("error creating pcli symlink")?;
        create_symlink(
            cache
                .get_pclientd_for_version(&self.pinned_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pclientd version found for version {}",
                        self.pinned_version
                    )
                })?,
            &self.pclientd_path(),
        )
        .context("error creating pclientd symlink")?;
        create_symlink(
            cache
                .get_pd_for_version(&self.pinned_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pd version found for version {}",
                        self.pinned_version
                    )
                })?,
            &self.pd_path(),
        )
        .context("error creating pd symlink")?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Environments {
    pub environments: Vec<Environment>,
}

impl Deref for Environments {
    type Target = Vec<Environment>;

    fn deref(&self) -> &Self::Target {
        &self.environments
    }
}

impl DerefMut for Environments {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.environments
    }
}

fn create_symlink(target: &Utf8PathBuf, link: &Utf8PathBuf) -> Result<()> {
    tracing::debug!("creating symlink from {} to {}", target, link);
    let metadata = fs::metadata(target)?;
    if !metadata.is_file() {
        return Err(anyhow!("symlink target must be a file"));
    }

    let link_metadata = fs::metadata(link);
    if link_metadata.is_ok() {
        tracing::debug!("symlink already existed, not recreating");
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        tracing::debug!("creating macos symlink from {} to {}", target, link);
        unix_symlink(target, link)?;

        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o111); // Add executable bit
        fs::set_permissions(link, permissions)?;
    }
    #[cfg(target_os = "unix")]
    {
        tracing::debug!("creating unix symlink from {} to {}", target, link);
        unix_symlink(target, link)?;

        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o111); // Add executable bit
        fs::set_permissions(link, permissions)?;
    }
    #[cfg(target_os = "windows")]
    {
        tracing::debug!("creating windows symlink from {} to {}", target, link);
        windows_symlink_file(target, link)?;
    }

    Ok(())
}
