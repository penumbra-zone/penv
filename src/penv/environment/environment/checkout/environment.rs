use anyhow::{Context as _, Result};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs::{self, File};
use std::io::{self, Write as _};
#[cfg(any(target_os = "macos", target_os = "unix"))]
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::Arc;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::penv::cache::cache::Cache;
use crate::penv::environment::{Binary as _, EnvironmentMetadata, EnvironmentTrait, ManagedFile};
use crate::penv::release::git_repo::CheckoutMetadata;
use crate::penv::release::RepoOrVersion;

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
        writeln!(f, "Git URL: {}", self.git_checkout.url)?;
        writeln!(f, "Root Directory: {}", self.metadata.root_dir)?;
        writeln!(f, "Include Node: {}", !self.metadata.client_only)?;
        writeln!(
            f,
            "Generated Dev Network: {}",
            self.metadata.generate_network
        )
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

impl EnvironmentTrait for CheckoutEnvironment {
    fn initialize(&self, _cache: &Cache) -> Result<()> {
        // Create the directory structure for the environment
        let bin_dir = self.path().join("bin");
        let checkout_dir = self.path().join("checkout");
        tracing::debug!("creating environment dir at {}", bin_dir);
        fs::create_dir_all(&bin_dir)
            .with_context(|| format!("Failed to create environment bin directory {}", bin_dir))?;

        // Copy the checkout into the environment
        tracing::debug!(
            "copying from {} to {}",
            self.git_checkout.install_path,
            checkout_dir
        );
        copy_dir_all(&self.git_checkout.install_path, &checkout_dir)?;

        // Handle writing the hook scripts into the bin dir
        let mut context = tera::Context::new();
        context.insert("checkout_dir", &checkout_dir.to_string());

        // TODO: this should live in some kind of Hook struct or trait or something
        // we don't have access to the current shell here unfortunately, so either
        // these are cross-shell or we need to pass the shell in
        // also the relative paths are kinda wild here, this should be abstracted
        let pcliwrapper_template = include_str!("../../../../../files/zsh-pcliwrapper.j2");
        let pcliwrapper = tera::Tera::one_off(pcliwrapper_template, &context, false)?;
        let pcli_binary = self.get_pcli_binary();

        let mut pcli_file = File::create(&pcli_binary.path())?;

        tracing::debug!(?pcliwrapper, "writing pcliwrapper");
        // Write the rendered pcli wrapper to the pcli binary location
        pcli_file.write_all(pcliwrapper.as_bytes())?;

        #[cfg(any(target_os = "macos", target_os = "unix"))]
        {
            let pcli_metadata = fs::metadata(&pcli_binary.path())?;
            let mut pcli_permissions = pcli_metadata.permissions();
            pcli_permissions.set_mode(pcli_permissions.mode() | 0o111); // Add executable bit
            fs::set_permissions(&pcli_binary.path(), pcli_permissions)?;
        }

        let pclientdwrapper_template = include_str!("../../../../../files/zsh-pclientdwrapper.j2");
        let pclientdwrapper = tera::Tera::one_off(pclientdwrapper_template, &context, false)?;
        let pclientd_binary = self.get_pclientd_binary();

        let mut pclientd_file = File::create(&pclientd_binary.path())?;

        pclientd_file.write_all(pclientdwrapper.as_bytes())?;

        #[cfg(any(target_os = "macos", target_os = "unix"))]
        {
            let pclientd_metadata = fs::metadata(&pclientd_binary.path())?;
            let mut pclientd_permissions = pclientd_metadata.permissions();
            pclientd_permissions.set_mode(pclientd_permissions.mode() | 0o111); // Add executable bit
            fs::set_permissions(&pclientd_binary.path(), pclientd_permissions)?;
        }

        if !self.metadata().client_only {
            context.insert("create_pd", "true");

            let pdwrapper_template = include_str!("../../../../../files/zsh-pdwrapper.j2");
            let pdwrapper = tera::Tera::one_off(pdwrapper_template, &context, false)?;
            let pd_binary = self.get_pd_binary();

            let mut pd_file = File::create(&pd_binary.path())?;

            pd_file.write_all(pdwrapper.as_bytes())?;

            #[cfg(any(target_os = "macos", target_os = "unix"))]
            {
                let pd_metadata = fs::metadata(&pd_binary.path())?;
                let mut pd_permissions = pd_metadata.permissions();
                pd_permissions.set_mode(pd_permissions.mode() | 0o111); // Add executable bit
                fs::set_permissions(&pd_binary.path(), pd_permissions)?;
            }
        }

        // If the environment is set to generate a local dev network,
        // we must initialize that prior to pcli and pclientd.
        // Initialize pcli configuration
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
