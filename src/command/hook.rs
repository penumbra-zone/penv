use std::env;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

#[derive(Debug, clap::Parser)]
pub struct HookCmd {
    /// The shell to output hook scripts for.
    // TODO: should be an enum
    shell: String,
}

// Supported shells for configuring pvm environments
pub enum Shell {
    Bash,
    Zsh,
}

// Implement String -> Shell conversion, to aid in parsing CLI args.
impl TryFrom<String> for Shell {
    type Error = anyhow::Error;
    fn try_from(s: String) -> Result<Shell> {
        if s == "bash" {
            Ok(Shell::Bash)
        } else if s == "zsh" {
            Ok(Shell::Zsh)
        } else {
            anyhow::bail!("unsupported shell: {}", s)
        }
    }
}

// Implement Shell -> String conversion, to aid in looking up relevant configs.
impl Into<String> for Shell {
    fn into(self) -> String {
        match self {
            Shell::Bash => String::from("bash"),
            Shell::Zsh => String::from("zsh"),
        }
    }
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

        let shell = Shell::try_from(self.shell.clone())?;
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
        }
        Ok(())
    }
}
