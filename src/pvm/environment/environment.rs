use camino::Utf8PathBuf;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Environment {
    pub alias: String,
    pub grpc_url: Url,
    pub version_requirement: VersionReq,
    pub pinned_version: Version,
    pub root_dir: Utf8PathBuf,
}
