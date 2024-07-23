use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use colored::Colorize;
use semver::VersionReq;
use url::Url;

use crate::pvm::Pvm;

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
    // Migrate will handle applying migrations between state-breaking versions
    // #[clap(flatten)]
    // Migrate(MigrateSubCmd),
    /// Upgrade a Penumbra environment to use the latest software version matching its semver version requirement.
    #[clap(display_order = 400)]
    Upgrade(UpgradeCmd),
    /// Display information about a specific Penumbra environment.
    #[clap(display_order = 500)]
    Info(InfoCmd),
    /// List all configured Penumbra environments.
    #[clap(display_order = 600)]
    List(ListCmd),
}

// TODO: it would be extremely useful to create an environment that can run
// against a local repository. This would probably work by symlinking out of the
// `target/` directory of the repository.

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
    /// Disable setting up a fullnode installation.
    #[clap(long)]
    client_only: bool,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct DeleteCmd {
    /// The alias of the Penumbra environment to be deleted.
    #[clap(display_order = 100)]
    environment_alias: String,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct ListCmd {
    /// Display detailed information about each environment instead of just the alias.
    #[clap(display_order = 100, long)]
    detailed: bool,
    // TODO: alias filter, pinned version filter, etc.
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

#[derive(Debug, Clone, clap::Parser)]
pub struct UpgradeCmd {
    /// The alias of the Penumbra environment to be upgraded.
    #[clap(display_order = 100)]
    environment_alias: String,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct InfoCmd {
    /// The alias of the Penumbra environment to print info about.
    #[clap(display_order = 100)]
    environment_alias: String,
}

impl ManageCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        match self {
            ManageCmd {
                subcmd:
                    ManageTopSubCmd::Create(CreateCmd {
                        environment_alias,
                        penumbra_version,
                        grpc_url,
                        repository_name,
                        client_only,
                    }),
            } => {
                let mut pvm = Pvm::new_from_repository(repository_name.clone(), home.clone())?;

                let env = pvm.create_environment(
                    environment_alias.clone(),
                    penumbra_version.clone(),
                    grpc_url.clone(),
                    repository_name.clone(),
                    client_only.clone(),
                )?;

                println!(
                    "created environment {} with pinned version {}",
                    environment_alias, env.pinned_version
                );

                Ok(())
            }
            ManageCmd {
                subcmd: ManageTopSubCmd::Delete(DeleteCmd { environment_alias }),
            } => {
                let mut pvm = Pvm::new(home.clone())?;

                pvm.delete_environment(environment_alias.clone())?;

                Ok(())
            }
            ManageCmd {
                subcmd: ManageTopSubCmd::Info(InfoCmd { environment_alias }),
            } => {
                let pvm = Pvm::new(home.clone())?;

                let info = pvm.environment_info(environment_alias.clone())?;

                println!("{}", info);

                Ok(())
            }
            ManageCmd {
                subcmd: ManageTopSubCmd::List(ListCmd { detailed }),
            } => {
                let pvm = Pvm::new(home.clone())?;

                let environments = pvm.environments()?;
                let active_environment = pvm.active_environment.clone();

                println!("Environments:");
                for environment in environments.iter() {
                    if *detailed {
                        if active_environment
                            .clone()
                            .is_some_and(|e| e.alias == environment.alias)
                        {
                            print!("{}", format!("{}Active: true\n\n", environment).green());
                        } else {
                            print!("{}", format!("{}Active: false\n\n", environment).red());
                        }
                    } else {
                        if active_environment
                            .clone()
                            .is_some_and(|e| e.alias == environment.alias)
                        {
                            print!("{}", format!("{} (active)\n\n", environment.alias).green());
                        } else {
                            print!("{}", format!("{}\n\n", environment.alias).red());
                        }
                    }
                }

                Ok(())
            }
            _ => Err(anyhow!("unimplemented")),
        }
    }
}
