use std::{collections::HashMap, process::Command};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8PathBuf;
use url::Url;

use super::{Binary, ManagedFile};

pub(crate) struct PdBinary {
    pub(crate) pd_join_url: Url,
    pub(crate) root_dir: Utf8PathBuf,
}

impl PdBinary {
    fn network_data_dir(&self) -> Utf8PathBuf {
        self.root_dir.join("network_data")
    }
}

impl ManagedFile for PdBinary {
    fn path(&self) -> Utf8PathBuf {
        self.root_dir.join("bin/pd")
    }
}

impl Binary for PdBinary {
    fn initialize(&self, configs: Option<HashMap<String, String>>) -> Result<String> {
        // TODO: support additional pd configuration here, for ex to generate
        // or to use a same/different seed phrase from the one configured for pcli/pclientd
        // pd network join \
        // --moniker MY_NODE_NAME \
        // --external-address IP_ADDRESS:26656 \
        // NODE_URL
        let configs = configs.context("configs should be set")?;
        let generate_network = configs.get("generate_network").is_some();
        let allocation_address = configs
            .get("allocation_address")
            .expect("allocation_address should be set");

        let pd_args = if generate_network {
            vec![
                "network".to_string(),
                "--network-dir".to_string(),
                self.network_data_dir().to_string(),
                "generate".to_string(),
                "--external-addresses".to_string(),
                configs
                    .get("external-address")
                    .context("external-address should be set")?
                    .to_string(),
                "--allocation-address".to_string(),
                allocation_address.to_string(),
            ]
        } else {
            vec![
                "network".to_string(),
                "--network-dir".to_string(),
                self.network_data_dir().to_string(),
                "join".to_string(),
                "--moniker".to_string(),
                configs
                    .get("moniker")
                    .context("moniker should be set")?
                    .to_string(),
                "--external-address".to_string(),
                configs
                    .get("external-address")
                    .context("external-address should be set")?
                    .to_string(),
                self.pd_join_url.to_string(),
                "--allocation-address".to_string(),
                allocation_address.to_string(),
            ]
        };
        // Execute the pd binary with the given arguments
        tracing::debug!(path=?self.path(), args=?pd_args, "executing pd binary");
        let output = Command::new(self.path()).args(pd_args).output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::debug!(?stdout, "command output");
            Ok(stdout.to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("pd failed with error:\n{}", stderr))
        }
    }
}
