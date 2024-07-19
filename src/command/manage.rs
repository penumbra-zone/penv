use semver::VersionReq;
use url::Url;

#[derive(Debug, clap::Parser)]
pub struct ManageCmd {
    #[clap(subcommand)]
    pub subcmd: ManageTopSubCmd,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ManageTopSubCmd {
    /// Configure and create a Penumbra environment (a set of software and configurations).
    #[clap(display_order = 100)]
    Create(CreateCmd),
    /// Delete a configured Penumbra environment.
    #[clap(display_order = 200)]
    Delete(DeleteCmd),
    /// Rename a configured Penumbra environment.
    #[clap(display_order = 300)]
    Rename(RenameCmd),
    // #[clap(flatten)]
    // Migrate(MigrateSubCmd),
}

#[derive(Debug, Clone, clap::Parser)]
pub struct CreateCmd {
    /// The alias of the Penumbra environment to be created.
    ///
    /// For example, if you create a local devnet environment on version 0.79.0, you might name it "v0.79.1-devnet".
    #[clap(display_order = 100)]
    environment_alias: String,
    /// The version of the Penumbra software suite to configure within the environment.
    ///
    /// Specified as a semver version requirement, i.e. "0.79" will use the latest 0.79.x release.
    ///
    /// If a matching version is not installed, pvm will attempt to install it.
    penumbra_version: VersionReq,
    /// The GRPC URL to use to connect to a fullnode.
    ///
    /// If pd configs are also being generated, this should typically be localhost:8080
    #[clap(parse(try_from_str = Url::parse))]
    grpc_url: Url,
    /// The GitHub repository to fetch releases from if an installation is necessary.
    ///
    /// Defaults to "penumbra-zone/penumbra"
    #[clap(long, default_value = "penumbra-zone/penumbra")]
    repository_name: String,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct DeleteCmd {
    /// The alias of the Penumbra environment to be deleted.
    #[clap(display_order = 100)]
    environment_alias: String,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct RenameCmd {
    /// The alias of the Penumbra environment to be renamed.
    #[clap(display_order = 100)]
    environment_alias: String,
    /// The new alias to rename the Penumbra environment.
    #[clap(display_order = 200)]
    new_alias: String,
}
