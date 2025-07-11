use std::{collections::HashMap, process::Command};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8PathBuf;
use regex::Regex;
use url::Url;

use super::{Binary, ManagedFile};

pub(crate) struct PcliBinary {
    pub(crate) grpc_url: Url,
    pub(crate) root_dir: Utf8PathBuf,
}

fn extract_seed_phrase(input: &str) -> Option<String> {
    // Define the regular expression pattern to match 12 or 24 words
    let pattern = r"YOUR PRIVATE SEED PHRASE \(SpendKey\):\s*\n\s*(\b\w+\b(?:\s+\b\w+\b){11}|\b\w+\b(?:\s+\b\w+\b){23})\s*\n\s*Save";
    let re = Regex::new(pattern).unwrap();

    // Search for the seed phrase in the input string
    if let Some(captures) = re.captures(input) {
        return captures.get(1).map(|m| m.as_str().to_string());
    }

    None
}

fn extract_address(input: &str) -> Option<String> {
    let pattern = r"^(penumbra1.*)\n$";
    let re = Regex::new(pattern).unwrap();

    // Search for the address in the input string
    if let Some(captures) = re.captures(input) {
        return captures.get(1).map(|m| m.as_str().to_string());
    }

    None
}

impl PcliBinary {
    pub fn pcli_data_dir(&self) -> Utf8PathBuf {
        self.root_dir.join("pcli")
    }
}

impl ManagedFile for PcliBinary {
    fn path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pcli")
    }
}

impl Binary for PcliBinary {
    fn initialize(&self, configs: Option<HashMap<String, String>>) -> Result<String> {
        let import_seed_phrase = configs
            .as_ref()
            .and_then(|c| c.get("import_seed_phrase"))
            .cloned();

        let mut pcli_args = vec![
            "--home".to_string(),
            self.pcli_data_dir().to_string(),
            "init".to_string(),
            "--grpc-url".to_string(),
            self.grpc_url.to_string(),
            "soft-kms".to_string(),
        ];

        if import_seed_phrase.is_some() {
            pcli_args.push("import-phrase".to_string());
        } else {
            pcli_args.push("generate".to_string());
        }
        // Execute the pcli binary with the given arguments
        tracing::debug!(path=?self.path(), args=?pcli_args, "executing pcli binary");

        if let Some(seed_phrase) = import_seed_phrase {
            // For import-phrase, we need to provide the seed phrase via stdin
            use std::io::Write;
            use std::process::Stdio;

            let mut child = Command::new(self.path())
                .args(pcli_args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(seed_phrase.as_bytes())?;
                stdin.write_all(b"\n")?;
            }

            let output = child.wait_with_output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                tracing::debug!(?stdout, "command output");
                // For import-phrase, return the imported seed phrase
                Ok(seed_phrase)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow!("Command failed with error:\n{}", stderr))
            }
        } else {
            // For generate, use the original logic
            let output = Command::new(self.path()).args(pcli_args).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // TODO: this will print the seed phrase to logging if that's the command you called
                // which is not always great
                tracing::debug!(?stdout, "command output");
                Ok(extract_seed_phrase(&stdout).context("Failed to extract seed phrase")?)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow!("Command failed with error:\n{}", stderr))
            }
        }
    }
}

impl PcliBinary {
    /// Returns the address associated with the given index, as a String.
    pub fn get_address(&self, index: u64) -> Result<String> {
        let pcli_args = vec![
            "--home".to_string(),
            self.pcli_data_dir().to_string(),
            "view".to_string(),
            "address".to_string(),
            index.to_string(),
        ];
        // Execute the pcli binary with the given arguments
        tracing::debug!(path=?self.path(), args=?pcli_args, "executing pcli binary");
        let output = Command::new(self.path()).args(pcli_args).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::debug!(?stdout, "command output");
            Ok(extract_address(&stdout).context("Failed to extract address")?)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Command failed with error:\n{}", stderr))
        }
    }
}
