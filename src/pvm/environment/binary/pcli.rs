use std::process::Command;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use url::Url;

use super::Binary;

pub(crate) struct PcliBinary {
    pub(crate) pcli_data_dir: Utf8PathBuf,
    pub(crate) grpc_url: Url,
    pub(crate) root_dir: Utf8PathBuf,
}

impl Binary for PcliBinary {
    fn path(&self) -> Utf8PathBuf {
        // TODO: this should probably only live here
        self.root_dir.join("bin/pcli")
    }

    fn initialize(&self) -> Result<()> {
        // TODO: support additional pcli configuration here, e.g. seed phrase, threshold, etc.
        let pcli_args = vec![
            "--home".to_string(),
            self.pcli_data_dir.to_string(),
            "init".to_string(),
            "--grpc-url".to_string(),
            self.grpc_url.to_string(),
            "soft-kms".to_string(),
            "generate".to_string(),
        ];
        // Execute the pcli binary with the given arguments
        tracing::debug!(path=?self.path(), args=?pcli_args, "executing pcli binary");
        let output = Command::new(self.path()).args(pcli_args).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // TODO: this will print the seed phrase to logging if that's the command you called
            // which is not always great
            tracing::debug!(?stdout, "command output");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Command failed with error:\n{}", stderr))?;
        }

        Ok(())
    }
}
