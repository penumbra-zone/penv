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
                .map(|e| e.alias.clone())
                .unwrap_or_default()
        );

        Ok(())
    }
}
