use std::{collections::HashMap, process::Command};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8PathBuf;
use url::Url;

use super::{Binary, ManagedFile};

pub(crate) struct PclientdBinary {
    pub(crate) grpc_url: Url,
    pub(crate) root_dir: Utf8PathBuf,
}

impl PclientdBinary {
    fn pclientd_data_dir(&self) -> Utf8PathBuf {
        self.root_dir.join("pclientd")
    }
}

impl ManagedFile for PclientdBinary {
    fn path(&self) -> Utf8PathBuf {
        // TODO: this should probably only live here
        self.root_dir.join("bin/pclientd")
    }
}

impl Binary for PclientdBinary {
    fn initialize(&self, configs: Option<HashMap<String, String>>) -> Result<String> {
        // TODO: support additional pclientd configuration here, e.g. seed phrase, threshold, etc.
        let pclientd_args = vec![
            "--home".to_string(),
            self.pclientd_data_dir().to_string(),
            "init".to_string(),
            "--grpc-url".to_string(),
            self.grpc_url.to_string(),
            "--custody".to_string(),
            configs
                .context("configs should be set")?
                .get("seed_phrase")
                .context("seed phrase should be set")?
                .to_string(),
        ];
        // Execute the pclientd binary with the given arguments
        tracing::debug!(path=?self.path(), args=?pclientd_args, "executing pclientd binary");
        let output = Command::new(self.path()).args(pclientd_args).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // TODO: this will print the seed phrase to logging if that's the command you called
            // which is not always great
            tracing::debug!(?stdout, "command output");
            Ok(stdout.to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("pclientd failed with error:\n{}", stderr))
        }
    }
}
