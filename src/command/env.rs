use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::pvm::Pvm;

use super::hook::Shell;

#[derive(Debug, clap::Parser)]
pub struct EnvCmd {
    /// Which shell environment to print configuration for.
    #[clap(default_value_t, value_enum)]
    shell: Shell,
}

impl EnvCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let pvm = Pvm::new(home.clone())?;

        match self.shell {
            Shell::Bash => self.print_bash(&pvm),
            Shell::Zsh => self.print_zsh(&pvm),
            _ => Err(anyhow!("unsupported shell: {:?}", self.shell)),
        }
    }

    fn print_zsh(&self, pvm: &Pvm) -> Result<()> {
        let mut context = tera::Context::new();

        if let Some(active_environment) = &pvm.active_environment {
            context.insert(
                "pvm_active_environment",
                &pvm.active_environment
                    .clone()
                    .map(|e| e.alias.clone())
                    .unwrap_or_default(),
            );
            if let Some(pcli_home) = pvm.pcli_home() {
                context.insert("pcli_home", &pcli_home);
            }
            if let Some(pclientd_home) = pvm.pclientd_home() {
                context.insert("pclientd_home", &pclientd_home);
            }
            if !active_environment.client_only {
                if let Some(pd_home) = pvm.pd_home() {
                    context.insert("pd_home", &pd_home);
                }
            }
        }

        context.insert("path_add", &pvm.path_string());

        let hook_template = include_str!("../../files/zsh-env.j2");
        let hook = tera::Tera::one_off(hook_template, &context, false)?;
        println!("{}", hook);

        Ok(())
    }

    fn print_bash(&self, pvm: &Pvm) -> Result<()> {
        // TODO: use a tera template here
        unimplemented!("bash env output not implemented yet");
    }
}
