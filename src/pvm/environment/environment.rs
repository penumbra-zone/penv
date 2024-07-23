use anyhow::{anyhow, Context as _, Result};
use std::fmt::{self, Display};
#[cfg(target_os = "windows")]
use std::os::windows::fs::symlink_file as windows_symlink_file;
use std::sync::Arc;
use std::{collections::HashMap, os::unix::fs::symlink as unix_symlink};
use std::{
    fs,
    ops::{Deref, DerefMut},
    os::unix::fs::PermissionsExt as _,
};

use camino::Utf8PathBuf;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::pvm::cache::cache::Cache;
use crate::pvm::environment::Binary as _;

use super::{PcliBinary, PclientdBinary, PdBinary};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Environment {
    pub alias: String,
    pub grpc_url: Url,
    pub version_requirement: VersionReq,
    // TODO: implement a way to update the pinned_version
    // to the latest matching the version_requirement
    pub pinned_version: Version,
    pub root_dir: Utf8PathBuf,
    pub pd_join_url: Url,
    // whether there should be a pd config generated as well
    // TODO: would be useful to be able to change this for an existing config
    // but requires special pd initialization step
    pub client_only: bool,
    /// Whether a dev network should be generated or an existing network should be joined.
    pub generate_network: bool,
}

impl Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Alias: {}", self.alias)?;
        writeln!(f, "GRPC URL: {}", self.grpc_url)?;
        writeln!(f, "Version Requirement: {}", self.version_requirement)?;
        writeln!(f, "Pinned Version: {}", self.pinned_version)?;
        writeln!(f, "Root Directory: {}", self.root_dir)?;
        writeln!(f, "Include Node: {}", !self.client_only)?;
        writeln!(f, "Generated Dev Network: {}", self.generate_network)
    }
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

        // Since the initialization is version-dependent, it is necessary
        // to shell out to the installed binary to perform the initialization.
        //
        // Create symlinks for the pinned version of the software stack
        self.create_symlinks(cache)?;

        // If the environment is set to generate a local dev network,
        // we must initialize that prior to pcli and pclientd.
        // Initialize pcli configuration
        let pcli_binary = self.get_pcli_binary();
        let seed_phrase = pcli_binary.initialize(None)?;
        // TODO: lol don't do this
        tracing::debug!("seed phrase: {}", seed_phrase);
        let pclientd_binary = self.get_pclientd_binary();
        pclientd_binary.initialize(Some(HashMap::from([(
            // pass the seed phrase here to avoid keeping in memory long-term
            "seed_phrase".to_string(),
            seed_phrase,
        )])))?;
        if !self.client_only {
            let pd_binary = self.get_pd_binary();
            let mut pd_configs = HashMap::from([
                (
                    "external-address".to_string(),
                    // TODO: make configurable
                    "0.0.0.0:26656".to_string(),
                ),
                ("moniker".to_string(), self.alias.to_string()),
            ]);

            if self.generate_network {
                pd_configs.insert("generate_network".to_string(), "true".to_string());
            }

            pd_binary.initialize(Some(pd_configs))?;
        }

        Ok(())
    }

    pub fn pcli_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pcli")
    }

    fn get_pcli_binary(&self) -> PcliBinary {
        PcliBinary {
            root_dir: self.root_dir.clone(),
            grpc_url: self.grpc_url.clone(),
        }
    }

    // TODO: just store these on the environment struct
    fn get_pclientd_binary(&self) -> PclientdBinary {
        PclientdBinary {
            root_dir: self.root_dir.clone(),
            grpc_url: self.grpc_url.clone(),
        }
    }

    fn get_pd_binary(&self) -> PdBinary {
        PdBinary {
            root_dir: self.root_dir.clone(),
            pd_join_url: self.pd_join_url.clone(),
        }
    }

    pub fn pclientd_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pclientd")
    }

    pub fn pd_path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pd")
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
        if !self.client_only {
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
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Environments {
    pub environments: Vec<Arc<Environment>>,
}

impl Deref for Environments {
    type Target = Vec<Arc<Environment>>;

    fn deref(&self) -> &Self::Target {
        &self.environments
    }
}

impl DerefMut for Environments {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.environments
    }
}

pub fn create_symlink(target: &Utf8PathBuf, link: &Utf8PathBuf) -> Result<()> {
    tracing::debug!("creating symlink from {} to {}", target, link);
    let metadata = fs::metadata(target)?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(anyhow!("symlink target must be a file or directory"));
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
