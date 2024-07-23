use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::pvm::Pvm;

#[derive(Debug, clap::Parser)]
pub struct EnvCmd {
    /// Which shell environment to print configuration for.
    shell: String,
}

impl EnvCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let pvm = Pvm::new(home.clone())?;
        if self.shell != "zsh" {
            return Err(anyhow!("unsupported shell: {}", self.shell));
        }

        // TODO: probably need to do a better job formatting/quoting these
        println!(
            "export PVM_ACTIVE_ENVIRONMENT=\"{}\"",
            pvm.active_environment
                .clone()
                .map(|e| e.alias.clone())
                .unwrap_or_default()
        );
        tracing::debug!("export PATH=\"$PATH:{}\"", pvm.path_string());
        println!("export PATH=\"$PATH:{}\"", pvm.path_string());

        // /// The home directory used to store pcli-related configuration and data.
        // #[clap(long, default_value_t = default_pcli_home(), env = "PENUMBRA_PCLI_HOME")]
        // pub pcli_home: Utf8PathBuf,
        // /// The home directory used to store pclientd-related state and data.
        // #[clap(long, default_value_t = default_pclientd_home(), env = "PENUMBRA_PCLIENTD_HOME")]
        // pub pclientd_home: Utf8PathBuf,

        Ok(())
    }
}
