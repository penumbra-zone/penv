use anyhow::Result;
use regex::Regex;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};
use target_lexicon::Triple;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RepoOrVersion {
    Version(Version),
    Repo(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RepoOrVersionReq {
    VersionReqOrLatest(VersionReqOrLatest),
    Repo(String),
}

impl RepoOrVersionReq {
    pub fn matches(&self, v: &Version, latest_version: &Version) -> bool {
        match self {
            RepoOrVersionReq::VersionReqOrLatest(version_req) => {
                version_req.matches(v, latest_version)
            }
            RepoOrVersionReq::Repo(_repo) => false,
        }
    }
}

impl FromStr for RepoOrVersionReq {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to parse as a VersionReqOrLatest...
        let version_or_latest = match VersionReqOrLatest::from_str(s) {
            Ok(version_or_latest) => Some(version_or_latest),
            Err(_) => None,
        };

        if let Some(version_or_latest) = version_or_latest {
            return Ok(Self::VersionReqOrLatest(version_or_latest));
        }

        // ...otherwise, treat as a repository path on disk or URL
        return Ok(Self::Repo(s.to_string()));
    }
}

impl Display for RepoOrVersionReq {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoOrVersionReq::Repo(repo) => write!(f, "{}", repo),
            RepoOrVersionReq::VersionReqOrLatest(version_req_or_latest) => {
                write!(f, "{}", version_req_or_latest)
            }
        }
    }
}

impl RepoOrVersion {
    pub fn matches(&self, v: &Version, _latest_version: &Version) -> bool {
        match self {
            RepoOrVersion::Version(version) => version == v,
            RepoOrVersion::Repo(_repo) => false,
        }
    }
}

impl FromStr for RepoOrVersion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Attempt to parse as a Version...
        let version = match Version::from_str(s) {
            Ok(version) => Some(version),
            Err(_) => None,
        };

        match version {
            Some(version) => Ok(Self::Version(version)),

            // ...otherwise, treat as a repository path on disk or URL
            None => Ok(Self::Repo(s.to_string())),
        }
    }
}

impl Display for RepoOrVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoOrVersion::Repo(repo) => write!(f, "{}", repo),
            RepoOrVersion::Version(version) => {
                write!(f, "{}", version)
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "args")]
pub enum VersionReqOrLatest {
    Latest,
    VersionReq(VersionReq),
}

impl VersionReqOrLatest {
    pub fn matches(&self, v: &Version, latest_version: &Version) -> bool {
        match self {
            VersionReqOrLatest::Latest => v == latest_version,
            VersionReqOrLatest::VersionReq(version) => version.matches(v),
        }
    }
}

impl FromStr for VersionReqOrLatest {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "latest" {
            Ok(VersionReqOrLatest::Latest)
        } else {
            match VersionReq::from_str(s) {
                Ok(version_req) => Ok(VersionReqOrLatest::VersionReq(version_req)),
                Err(_) => Err("Invalid version requirement"),
            }
        }
    }
}

impl Display for VersionReqOrLatest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionReqOrLatest::Latest => write!(f, "latest"),
            VersionReqOrLatest::VersionReq(version_req) => write!(f, "{}", version_req),
        }
    }
}

pub(crate) fn extract_triple(filename: &str) -> Option<Triple> {
    // Define the regex pattern to capture the target triple part of the filename
    let re = Regex::new(r"-([a-zA-Z0-9_-]+)-([a-zA-Z0-9_-]+)-([a-zA-Z0-9_-]+)\.").unwrap();
    tracing::debug!("extracting triple from filename: {}", filename);

    // Apply the regex to the filename
    if let Some(captures) = re.captures(filename) {
        // Reconstruct the target triple from the captured groups
        let triple_str = format!("{}-{}-{}", &captures[1], &captures[2], &captures[3]);
        tracing::debug!("triple_str: {}", triple_str);

        // Parse the target triple string into a Triple
        match Triple::from_str(&triple_str) {
            Ok(triple) => return Some(triple),
            Err(e) => {
                tracing::debug!("failed to parse triple: {}", e);
                return None;
            }
        }
    }

    None
}
