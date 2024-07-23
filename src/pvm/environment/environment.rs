use anyhow::{anyhow, Context as _, Result};
use pcli;
#[cfg(target_os = "windows")]
use std::os::windows::fs::symlink_file as windows_symlink_file;
use std::{
    fs,
    ops::{Deref, DerefMut},
    os::unix::fs::PermissionsExt as _,
    process::Command,
};
use std::{io, os::unix::fs::symlink as unix_symlink};

use camino::Utf8PathBuf;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::pvm::cache::cache::Cache;

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
        create_symlink(
            cache
                .get_pcli_for_version(&self.pinned_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed version found for version {}",
                        self.pinned_version
                    )
                })?,
            &self.pcli_path(),
        )
        .context("error creating symlink")?;

        // Initialize pcli configuration
        // Since the initialization is version-dependent, it is necessary
        // to shell out to the installed binary to perform the initialization.
        // TODO: support additional pcli configuration here, e.g. seed phrase, threshold, etc.
        let pcli_args = vec![
            "--home".to_string(),
            self.pcli_data_dir().to_string(),
            "init".to_string(),
            "--grpc-url".to_string(),
            self.grpc_url.to_string(),
            "soft-kms".to_string(),
            "generate".to_string(),
        ];
        // Execute the pcli binary with the given arguments
        let output = Command::new(self.pcli_path()).args(pcli_args).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // TODO: this will print the seed phrase to logging if that's the command you called
            // which is not always great
            tracing::debug!(?stdout, "command output");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Command failed with error:\n{}", stderr))?;
        }

        Ok(())
    }

    pub fn pcli_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pcli")
    }

    pub fn pcli_data_dir(&self) -> Utf8PathBuf {
        self.root_dir.join("pcli")
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
