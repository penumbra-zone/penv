use std::fmt::{self, Display};

use anyhow::Result;
use binary::InstalledBinaryRelease;
use camino::Utf8PathBuf;
use git_repo::{CheckoutMetadata, RepoMetadata};
use semver::Version;
use serde::{Deserialize, Serialize};
use target_lexicon::Triple;

use super::{Asset, InstalledAsset, RawAsset, RepoOrVersion};

pub(crate) mod binary;
pub(crate) mod git_repo;

/// Release information as deserialized from the GitHub API JSON,
/// prior to enriching.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RawRelease {
    tag_name: String,
    name: String,
    body: Option<String>,
    assets: Vec<RawAsset>,
}

/// Release information enriched with proper domain types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The version of the release, parsed as semver.
    pub version: Version,
    /// The markdown formatted release notes.
    pub body: Option<String>,
    /// The collection of assets associated with the release, for all architectures.
    pub assets: Vec<Asset>,
    /// The name of the release on GitHub.
    pub name: String,
}

impl Display for Release {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

/// Defines a trait for using installed releases.
pub trait UsableRelease {
    fn assets(&self) -> &[InstalledAsset];
    fn uninstall(self) -> Result<()>;
}

impl UsableRelease for InstalledRelease {
    fn assets(&self) -> &[InstalledAsset] {
        match self {
            InstalledRelease::Binary(release) => &release.assets,
            InstalledRelease::GitCheckout(checkout) => checkout.assets(),
        }
    }

    fn uninstall(self) -> Result<()> {
        match self {
            InstalledRelease::Binary(release) => release.uninstall(),
            InstalledRelease::GitCheckout(checkout) => checkout.uninstall(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "args")]
pub enum InstalledRelease {
    Binary(InstalledBinaryRelease),
    GitCheckout(CheckoutMetadata),
}

impl InstalledRelease {
    pub fn matches(&self, repo_or_version: &RepoOrVersion) -> bool {
        match self {
            InstalledRelease::Binary(release) => match repo_or_version {
                RepoOrVersion::Repo(_) => false,
                RepoOrVersion::Version(version) => release.version == *version,
            },
            InstalledRelease::GitCheckout(checkout) => match repo_or_version {
                RepoOrVersion::Repo(repo) => checkout.url == *repo,
                RepoOrVersion::Version(_) => false,
            },
        }
    }
}

impl Display for InstalledRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstalledRelease::Binary(release) => write!(f, "{}", release),
            InstalledRelease::GitCheckout(repo) => write!(f, "{}", repo),
        }
    }
}

impl TryInto<Release> for RawRelease {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Release> {
        let version = Version::parse(&self.tag_name[1..])?;
        Ok(Release {
            version,
            body: self.body,
            assets: self
                .assets
                .into_iter()
                .map(|a| a.try_into())
                .collect::<Result<_>>()?,
            name: self.name,
        })
    }
}

impl TryInto<Release> for &RawRelease {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Release> {
        let version = Version::parse(&self.tag_name[1..])?;
        Ok(Release {
            version,
            body: self.body.clone(),
            assets: self
                .assets
                .clone()
                .into_iter()
                .map(|a| a.try_into())
                .collect::<Result<_>>()?,
            name: self.name.clone(),
        })
    }
}

impl PartialOrd for Release {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.version.partial_cmp(&other.version)
    }
}

impl Ord for Release {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version.cmp(&other.version)
    }
}

// A trait to commonalize the API between git checkouts and binary installs
pub trait Installable {
    fn version(&self) -> Option<&Version>;
    fn install(&self, install_path: Utf8PathBuf) -> Result<InstalledRelease>;
}

impl PartialOrd for InstallableRelease {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.version().partial_cmp(&other.version())
    }
}

impl Ord for InstallableRelease {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version().cmp(&other.version())
    }
}

impl Installable for InstallableRelease {
    fn version(&self) -> Option<&Version> {
        match self {
            InstallableRelease::GitRepo(_metadata) => None,
            InstallableRelease::Binary(release) => Some(release.version()),
        }
    }

    fn install(&self, install_path: Utf8PathBuf) -> Result<InstalledRelease> {
        match self {
            InstallableRelease::GitRepo(metadata) => metadata.install(install_path),
            InstallableRelease::Binary(release) => release.install(install_path),
        }
    }
}

/// Consists of the individual installable assets from a given release for the
/// desired architecture.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InstallableRelease {
    GitRepo(RepoMetadata),
    Binary(InstallableBinaryRelease),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InstallableBinaryRelease {
    pub(crate) pcli: Option<Utf8PathBuf>,
    pub(crate) pclientd: Option<Utf8PathBuf>,
    pub(crate) pd: Option<Utf8PathBuf>,
    pub(crate) target_arch: Triple,
    /// The underlying release information.
    pub(crate) release: Release,
}

impl InstallableBinaryRelease {
    pub fn version(&self) -> &Version {
        &self.release.version
    }
}
