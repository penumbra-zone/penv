use anyhow::{anyhow, Result};
use std::fmt::{self, Display};
#[cfg(any(target_os = "macos", target_os = "unix"))]
use std::os::unix::fs::symlink as unix_symlink;
#[cfg(any(target_os = "macos", target_os = "unix"))]
use std::os::unix::fs::PermissionsExt as _;
#[cfg(target_os = "windows")]
use std::os::windows::fs::symlink_file as windows_symlink_file;
use std::sync::Arc;
use std::{
    fs,
    ops::{Deref, DerefMut},
};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::penv::cache::cache::Cache;
use crate::penv::release::RepoOrVersion;

use super::{ManagedFile, PcliBinary, PclientdBinary, PdBinary};

// The two environment types are binary and checkout.
mod binary;
mod checkout;

pub use binary::*;
pub use checkout::*;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "type", content = "args")]
pub enum Environment {
    CheckoutEnvironment(CheckoutEnvironment),
    BinaryEnvironment(BinaryEnvironment),
}

impl ManagedFile for Environment {
    fn path(&self) -> Utf8PathBuf {
        match self {
            Environment::CheckoutEnvironment(env) => env.path(),
            Environment::BinaryEnvironment(env) => env.path(),
        }
    }
}

impl Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Environment::CheckoutEnvironment(env) => write!(f, "{}", env),
            Environment::BinaryEnvironment(env) => write!(f, "{}", env),
        }
    }
}

impl EnvironmentTrait for Environment {
    fn initialize(&self, cache: &Cache) -> Result<()> {
        match self {
            Environment::CheckoutEnvironment(env) => env.initialize(cache),
            Environment::BinaryEnvironment(env) => env.initialize(cache),
        }
    }

    fn create_symlinks(&self, cache: &Cache) -> Result<()> {
        match self {
            // no symlinks are created for git checkout environments
            Environment::CheckoutEnvironment(_env) => Ok(()),
            Environment::BinaryEnvironment(env) => env.create_symlinks(cache),
        }
    }

    fn satisfied_by_version(&self, version: &RepoOrVersion) -> bool {
        match self {
            Environment::CheckoutEnvironment(_) => false,
            Environment::BinaryEnvironment(env) => env.satisfied_by_version(version),
        }
    }

    fn metadata(&self) -> &EnvironmentMetadata {
        match self {
            Environment::CheckoutEnvironment(env) => &env.metadata,
            Environment::BinaryEnvironment(env) => &env.metadata,
        }
    }
}

/// Fields common to all environment types.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct EnvironmentMetadata {
    pub alias: String,
    pub grpc_url: Url,
    pub root_dir: Utf8PathBuf,
    pub pd_join_url: Url,
    // whether there should be a pd config generated as well
    // TODO: would be useful to be able to change this for an existing config
    // but requires special pd initialization step
    pub client_only: bool,
    /// Whether a dev network should be generated or an existing network should be joined.
    pub generate_network: bool,
}

pub trait EnvironmentTrait: ManagedFile {
    /// Initializes an environment on disk, by creating the necessary
    /// pd/pclientd/pcli configurations and symlinks to the
    /// pinned version of the software stack.
    fn initialize(&self, cache: &Cache) -> Result<()>;

    fn get_pcli_binary(&self) -> PcliBinary {
        PcliBinary {
            root_dir: self.path().clone(),
            grpc_url: self.metadata().grpc_url.clone(),
        }
    }

    fn get_pclientd_binary(&self) -> PclientdBinary {
        PclientdBinary {
            root_dir: self.path().clone(),
            grpc_url: self.metadata().grpc_url.clone(),
        }
    }

    fn get_pd_binary(&self) -> PdBinary {
        PdBinary {
            root_dir: self.path().clone(),
            pd_join_url: self.metadata().pd_join_url.clone(),
        }
    }

    fn pcli_path(&self) -> Utf8PathBuf {
        self.path().join("bin/pcli")
    }

    fn pclientd_path(&self) -> Utf8PathBuf {
        self.path().join("bin/pclientd")
    }

    fn pd_path(&self) -> Utf8PathBuf {
        self.path().join("bin/pd")
    }

    fn create_symlinks(&self, cache: &Cache) -> Result<()>;

    fn satisfied_by_version(&self, version: &RepoOrVersion) -> bool;

    fn metadata(&self) -> &EnvironmentMetadata;
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

    #[cfg(any(target_os = "macos", target_os = "unix"))]
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
