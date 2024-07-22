use anyhow::Result;
use regex::Regex;
use semver::{Version, VersionReq};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};
use target_lexicon::Triple;

#[derive(Debug, Clone)]
pub enum VersionOrLatest {
    Latest,
    VersionReq(VersionReq),
}

impl VersionOrLatest {
    pub fn matches(&self, v: &Version, latest_version: &Version) -> bool {
        match self {
            VersionOrLatest::Latest => v == latest_version,
            VersionOrLatest::VersionReq(version) => version.matches(v),
        }
    }
}

impl FromStr for VersionOrLatest {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "latest" {
            Ok(VersionOrLatest::Latest)
        } else {
            match VersionReq::from_str(s) {
                Ok(version_req) => Ok(VersionOrLatest::VersionReq(version_req)),
                Err(_) => Err("Invalid version requirement"),
            }
        }
    }
}

impl Display for VersionOrLatest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionOrLatest::Latest => write!(f, "latest"),
            VersionOrLatest::VersionReq(version_req) => write!(f, "{}", version_req),
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
