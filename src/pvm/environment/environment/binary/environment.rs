use anyhow::{Context as _, Result};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs;

use camino::Utf8PathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::pvm::{
    cache::cache::Cache,
    environment::{
        create_symlink, Binary as _, EnvironmentMetadata, EnvironmentTrait, ManagedFile,
    },
    release::{RepoOrVersion, VersionReqOrLatest},
};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BinaryEnvironment {
    /// Fields common to all environment types.
    pub metadata: EnvironmentMetadata,
    /// The version_requirement is only set for binary releases.
    ///
    /// For git checkouts, there is no version -- the state of the checkout
    /// defines the code that will run.
    pub version_requirement: VersionReqOrLatest,
    // TODO: implement a way to update the pinned_version
    // to the latest matching the version_requirement
    /// The pinned_version is only set for binary releases.
    ///
    /// For git checkouts, there is no version -- the state of the checkout
    /// defines the code that will run.
    pub pinned_version: Version,
}

impl ManagedFile for BinaryEnvironment {
    fn path(&self) -> Utf8PathBuf {
        self.metadata.root_dir.clone()
    }
}

impl Display for BinaryEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Alias: {}", self.metadata.alias)?;
        writeln!(f, "GRPC URL: {}", self.metadata.grpc_url)?;
        writeln!(f, "Version Requirement: {}", self.version_requirement)?;
        writeln!(f, "Pinned Version: {}", self.pinned_version)?;
        writeln!(f, "Root Directory: {}", self.metadata.root_dir)?;
        writeln!(f, "Include Node: {}", !self.metadata.client_only)?;
        writeln!(
            f,
            "Generated Dev Network: {}",
            self.metadata.generate_network
        )
    }
}

impl EnvironmentTrait for BinaryEnvironment {
    /// Initializes an environment on disk, by creating the necessary
    /// pd/pclientd/pcli configurations and symlinks to the
    /// pinned version of the software stack.
    fn initialize(&self, cache: &Cache) -> Result<()> {
        // Create the directory structure for the environment
        let bin_dir = self.path().join("bin");
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
        if !self.metadata().client_only {
            let pd_binary = self.get_pd_binary();
            let mut pd_configs = HashMap::from([
                (
                    "external-address".to_string(),
                    // TODO: make configurable
                    "0.0.0.0:26656".to_string(),
                ),
                ("moniker".to_string(), self.metadata().alias.to_string()),
            ]);

            if self.metadata().generate_network {
                pd_configs.insert("generate_network".to_string(), "true".to_string());
            }

            pd_binary.initialize(Some(pd_configs))?;
        }

        Ok(())
    }

    fn create_symlinks(&self, cache: &Cache) -> Result<()> {
        let pinned_version = &self.pinned_version;

        create_symlink(
            cache.get_pcli_for_version(&pinned_version).ok_or_else(|| {
                anyhow::anyhow!(
                    "No installed pcli version found for version {}",
                    pinned_version
                )
            })?,
            &self.pcli_path(),
        )
        .context("error creating pcli symlink")?;
        create_symlink(
            cache
                .get_pclientd_for_version(&pinned_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pclientd version found for version {}",
                        pinned_version
                    )
                })?,
            &self.pclientd_path(),
        )
        .context("error creating pclientd symlink")?;
        if !self.metadata().client_only {
            create_symlink(
                cache.get_pd_for_version(&pinned_version).ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pd version found for version {}",
                        pinned_version
                    )
                })?,
                &self.pd_path(),
            )
            .context("error creating pd symlink")?;
        }

        Ok(())
    }

    fn satisfied_by_version(&self, version: &RepoOrVersion) -> bool {
        match (&self.version_requirement, version) {
            (VersionReqOrLatest::VersionReq(version_req), RepoOrVersion::Version(version)) => {
                version_req.matches(version)
            }
            (VersionReqOrLatest::Latest, RepoOrVersion::Version(_version)) => {
                unimplemented!("don't have latest version here")
            }
            // Latest never satisfied by a checkout
            (VersionReqOrLatest::Latest, RepoOrVersion::Repo(_repo)) => false,
            // A checkout environment is never satisfied by a binary version
            (VersionReqOrLatest::VersionReq(_version_req), RepoOrVersion::Repo(_repo)) => false,
        }
    }

    fn metadata(&self) -> &EnvironmentMetadata {
        &self.metadata
    }
}
