use cache::CacheCmd;
use env::EnvCmd;
use install::InstallCmd;
use list::ListCmd;
use manage::ManageCmd;
use use_::UseCmd;
use which::WhichCmd;

mod cache;
mod env;
mod install;
mod list;
mod manage;
mod use_;
mod which;

// TODO: it would be cool to support migrations in here eventually, for example
// initializing an environment on version 0.79 and then upgrading it to 0.80
//
// for now, that will remain a manual process.

#[derive(Debug, clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    /// Install a given version of the Penumbra software suite.
    #[clap(display_order = 100, visible_alias = "i")]
    Install(InstallCmd),
    /// Set a Penumbra environment as active in the current shell.
    #[clap(display_order = 200, visible_alias = "u")]
    Use(UseCmd),
    /// Manage the cache of installed Penumbra versions.
    #[clap(display_order = 300, visible_alias = "c")]
    Cache(CacheCmd),
    /// List the Penumbra versions available for installation.
    #[clap(display_order = 400, visible_alias = "ls")]
    List(ListCmd),
    /// Manage an installed Penumbra environment, for example to rename or delete it.
    #[clap(display_order = 500, visible_alias = "m")]
    Manage(ManageCmd),
    /// Display information about the active Penumbra environment.
    #[clap(display_order = 600, visible_alias = "w")]
    Which(WhichCmd),
    /// Output the necessary environment variables to use pvm.
    #[clap(display_order = 700, visible_alias = "e")]
    Env(EnvCmd),
}
