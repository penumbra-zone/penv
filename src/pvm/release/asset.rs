use anyhow::Result;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use target_lexicon::Triple;

use super::extract_triple;

/// Asset information as deserialized from the GitHub API JSON,
/// prior to enriching.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RawAsset {
    url: String,
    id: u64,
    name: String,
    content_type: String,
    state: String,
    size: u64,
    created_at: String,
    updated_at: String,
    browser_download_url: String,
}

/// Asset information enriched with proper domain types.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Asset {
    pub target_arch: Option<Triple>,
    pub browser_download_url: String,
    pub expected_sha256sum: Option<String>,
}

impl TryInto<Asset> for RawAsset {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Asset> {
        // fetch target_arch from the name field
        let target_arch = extract_triple(&self.name);

        Ok(Asset {
            target_arch,
            browser_download_url: self.browser_download_url,
            expected_sha256sum: None,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledAsset {
    pub target_arch: Triple,
    pub local_filepath: Utf8PathBuf,
}
