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
        println!("export PATH=\"{}:$PATH\"", pvm.path_string());

        if let Some(pcli_home) = pvm.pcli_home() {
            println!("export PENUMBRA_PCLI_HOME=\"{}\"", pcli_home);
        }
        if let Some(pclientd_home) = pvm.pcli_home() {
            println!("export PENUMBRA_PCLIENTD_HOME=\"{}\"", pclientd_home);
        }
        if let Some(active_environment) = &pvm.active_environment {
            if !active_environment.client_only {
                if let Some(pd_home) = pvm.pd_home() {
                    println!("export PENUMBRA_PD_HOME=\"{}\"", pd_home);
                }
            }
        }

        Ok(())
    }
}
