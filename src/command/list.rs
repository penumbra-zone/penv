use semver::VersionReq;

#[derive(Debug, clap::Parser)]
pub struct ListCmd {
    /// Only list releases that meet the given semver version requirement.
    #[clap(long)]
    required_version: Option<VersionReq>,
}
