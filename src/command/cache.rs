use semver::{Version, VersionReq};

#[derive(Debug, clap::Parser)]
pub struct CacheCmd {
    #[clap(subcommand)]
    pub subcmd: CacheTopSubCmd,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum CacheTopSubCmd {
    #[clap(display_order = 100)]
    List(ListCmd),
    #[clap(display_order = 200)]
    Delete(DeleteCmd),
}

#[derive(Debug, Clone, clap::Parser)]
pub struct ListCmd {
    /// Only list versions matching the given semver version requirement.
    required_version: Option<VersionReq>,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct DeleteCmd {
    /// The version to delete.
    version: Version,
}
