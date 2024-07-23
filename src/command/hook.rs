use std::env;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;

#[derive(Debug, clap::Parser)]
pub struct HookCmd {
    /// The shell to output hook scripts for.
    // TODO: should be an enum
    shell: String,
}

impl HookCmd {
    pub async fn exec(&self, _home: Utf8PathBuf) -> Result<()> {
        if self.shell != "zsh" {
            anyhow::bail!("unsupported shell: {}", self.shell);
        }

        let current_exe = match env::current_exe() {
            Ok(exe_path) => exe_path,
            Err(e) => return Err(anyhow!("failed to get current exe path: {e}")),
        };

        println!(
            "{}{}{}",
            r#"
_pvm_hook() {
  trap -- '' SIGINT
  eval "$(""#,
            current_exe.display(),
            r#"" env zsh)"
  trap - SIGINT
}
typeset -ag precmd_functions
if (( ! ${precmd_functions[(I)_pvm_hook]} )); then
  precmd_functions=(_pvm_hook $precmd_functions)
fi
        "#
        );

        Ok(())
    }
}
