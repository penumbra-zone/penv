use semver::VersionReq;

// TODO: the cometbft version being used must be matched to the penumbra versions,
// so it might be desirable to add support for managing cometbft installations here as well.

#[derive(Debug, clap::Parser)]
pub struct InstallCmd {
    /// The GitHub repository to fetch releases from.
    ///
    /// Defaults to "penumbra-zone/penumbra"
    #[clap(long, default_value = "penumbra-zone/penumbra")]
    repository_name: String,
    /// The version of the Penumbra software suite to install.
    ///
    /// Specified as a semver version requirement, i.e. "0.79" will install the latest 0.79.x release.
    penumbra_version: VersionReq,
}
