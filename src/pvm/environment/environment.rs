use std::ops::{Deref, DerefMut};

use camino::Utf8PathBuf;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Environment {
    pub alias: String,
    pub grpc_url: Url,
    pub version_requirement: VersionReq,
    // TODO: implement a way to update the pinned_version
    // to the latest matching the version_requirement
    pub pinned_version: Version,
    pub root_dir: Utf8PathBuf,
    // TODO: include whether there should be a pd config generated as well
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Environments {
    pub environments: Vec<Environment>,
}

impl Deref for Environments {
    type Target = Vec<Environment>;

    fn deref(&self) -> &Self::Target {
        &self.environments
    }
}

impl DerefMut for Environments {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.environments
    }
}
