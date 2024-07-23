use std::env;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use serde::Serialize;

#[derive(Debug, clap::Parser)]
pub struct HookCmd {
    /// The shell to output hook scripts for.
    #[clap(default_value_t, value_enum)]
    shell: Shell,
}

// Supported shells for configuring pvm environments
#[derive(Debug, Clone, Default, clap::ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shell {
    Bash,
    Zsh,
    #[default]
    Unsupported,
}

impl HookCmd {
    pub async fn exec(&self, _home: Utf8PathBuf) -> Result<()> {
        // Look up the path for the currently executed `pvm`, so we can use the fullpath
        // in the shell configs. This ensures that the sourced shell will work,
        // even if pvm isn't on PATH already.
        let current_exe = match env::current_exe() {
            Ok(exe_path) => exe_path,
            Err(e) => return Err(anyhow!("failed to get current exe path: {e}")),
        };

        // Load Tera template config, so we can interpolate pvm fullpath in shell hooks.
        let mut context = tera::Context::new();
        context.insert("pvm_executable", &current_exe);

        let shell = self.shell.clone();
        match shell {
            Shell::Zsh => {
                let hook_template = include_str!("../../files/zsh-hook.j2");
                let hook = tera::Tera::one_off(hook_template, &context, false)?;
                println!("{}", hook);
            }
            Shell::Bash => {
                let hook_template = include_str!("../../files/bash-hook.j2");
                let hook = tera::Tera::one_off(hook_template, &context, false)?;
                println!("{}", hook);
            }
            Shell::Unsupported => {
                return Err(anyhow!("please provide a supported shell: `zsh` or `bash`"))
            }
        }
        Ok(())
    }
}
