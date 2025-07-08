use std::{
    collections::HashMap,
    io::Write,
    process::{Command, Stdio},
};

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

        let seed_phrase = configs
            .context("configs should be set")?
            .get("seed_phrase")
            .context("seed phrase should be set")?
            .to_string();

        let pclientd_args = vec![
            "--home".to_string(),
            self.pclientd_data_dir().to_string(),
            "init".to_string(),
            "--grpc-url".to_string(),
            self.grpc_url.to_string(),
        ];

        // Execute the pclientd binary with stdin for custody value
        tracing::debug!(path=?self.path(), args=?pclientd_args, "executing pclientd binary");
        let mut child = Command::new(self.path())
            .args(pclientd_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write the seed phrase to stdin
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(seed_phrase.as_bytes())?;
        }

        let output = child.wait_with_output()?;

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
