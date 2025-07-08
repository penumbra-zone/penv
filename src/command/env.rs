use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

use crate::penv::environment::EnvironmentTrait as _;
use crate::penv::Penv;

use super::hook::Shell;

#[derive(Debug, clap::Parser)]
pub struct EnvCmd {
    /// Which shell environment to print configuration for.
    #[clap(default_value_t, value_enum)]
    shell: Shell,
}

impl EnvCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let penv = Penv::new(home.clone())?;

        let mut context = tera::Context::new();

        if let Some(active_environment) = &penv.active_environment {
            context.insert(
                "penv_active_environment",
                &penv
                    .active_environment
                    .clone()
                    .map(|e| e.metadata().alias.clone())
                    .unwrap_or_default(),
            );
            if let Some(pcli_home) = penv.pcli_home() {
                context.insert("pcli_home", &pcli_home);
            }
            if let Some(pclientd_home) = penv.pclientd_home() {
                context.insert("pclientd_home", &pclientd_home);
            }
            if !active_environment.metadata().client_only {
                if let Some(pd_home) = penv.pd_home() {
                    context.insert("pd_home", &pd_home);
                }

                context.insert("pd_join_url", &active_environment.metadata().pd_join_url);
                context.insert(
                    "pd_cometbft_proxy_url",
                    &active_environment.metadata().pd_join_url,
                );
                context.insert("cometbft_home", &penv.cometbft_home());
            }
        }

        context.insert("path_add", &penv.path_string());

        match self.shell {
            Shell::Bash => self.print_bash(&context),
            Shell::Zsh => self.print_zsh(&context),
            Shell::Unsupported => Err(anyhow!("please provide a supported shell: `zsh` or `bash`")),
        }
    }

    fn print_zsh(&self, context: &tera::Context) -> Result<()> {
        let hook_template = include_str!("../../files/zsh-env.j2");
        let hook = tera::Tera::one_off(hook_template, context, false)?;
        println!("{}", hook);

        Ok(())
    }

    fn print_bash(&self, context: &tera::Context) -> Result<()> {
        let hook_template = include_str!("../../files/bash-env.j2");
        let hook = tera::Tera::one_off(hook_template, context, false)?;
        println!("{}", hook);

        Ok(())
    }
}
