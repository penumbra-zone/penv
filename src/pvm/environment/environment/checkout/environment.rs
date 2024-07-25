use anyhow::{Context as _, Result};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs;
use std::sync::Arc;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::pvm::cache::cache::Cache;
use crate::pvm::environment::{Binary as _, EnvironmentMetadata, EnvironmentTrait, ManagedFile};
use crate::pvm::release::git_repo::CheckoutMetadata;
use crate::pvm::release::RepoOrVersion;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct CheckoutEnvironment {
    /// Fields common to all environment types.
    pub metadata: EnvironmentMetadata,
    // A reference to the InstalledRelease that this environment is based on.
    // TODO: probably doesn't need to be an Arc
    pub git_checkout: Arc<CheckoutMetadata>,
}

impl ManagedFile for CheckoutEnvironment {
    fn path(&self) -> Utf8PathBuf {
        self.metadata.root_dir.clone()
    }
}

impl Display for CheckoutEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Alias: {}", self.metadata.alias)?;
        writeln!(f, "GRPC URL: {}", self.metadata.grpc_url)?;
        writeln!(f, "Git Checkout: true")?;
        // TODO: print parent checkout here?
        writeln!(f, "Root Directory: {}", self.metadata.root_dir)?;
        writeln!(f, "Include Node: {}", !self.metadata.client_only)?;
        writeln!(
            f,
            "Generated Dev Network: {}",
            self.metadata.generate_network
        )
    }
}

impl EnvironmentTrait for CheckoutEnvironment {
    fn initialize(&self, _cache: &Cache) -> Result<()> {
        // Create the directory structure for the environment
        let root_dir = self.path();
        tracing::debug!("creating environment dir at {}", root_dir);
        fs::create_dir_all(&root_dir)
            .with_context(|| format!("Failed to create environment directory {}", root_dir))?;

        // TODO: need to handle the hooks so we can initialize the binaries properly

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

    fn satisfied_by_version(&self, version: &RepoOrVersion) -> bool {
        match version {
            RepoOrVersion::Version(_version) => {
                // a checkout environment is never satisfied by a binary version
                false
            }
            RepoOrVersion::Repo(repo) => repo == &self.git_checkout.url,
        }
    }

    fn metadata(&self) -> &EnvironmentMetadata {
        &self.metadata
    }

    fn create_symlinks(&self, _cache: &Cache) -> Result<()> {
        // Nothing to do here, the hook will handle setting the aliases
        Ok(())
    }
}
