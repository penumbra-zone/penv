use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::value_parser;
use colored::Colorize;
use url::Url;

use crate::penv::{
    environment::{Environment, EnvironmentTrait as _, ManagedFile as _},
    release::{InstalledRelease, RepoOrVersionReq},
    Penv,
};

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
    /// Reset the application state of an environment.
    Reset(ResetCmd),
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
    /// If a matching version is not installed, penv will attempt to install it.
    #[clap(long, value_parser = value_parser!(RepoOrVersionReq))]
    penumbra_version: RepoOrVersionReq,
    /// The GRPC URL to use to connect to a fullnode.
    ///
    /// If pd configs are also being generated, this should typically be localhost:8080
    #[clap(long, parse(try_from_str = Url::parse))]
    grpc_url: Url,
    /// The URL to use for `pd network join` operations, aka the cometBFT RPC endpoint.
    ///
    /// Typically this runs on port 26657. If not supplied, this will
    /// default to the GRPC URL with the port changed to 26657 and HTTP protocol.
    #[clap(long, parse(try_from_str = Url::parse))]
    pd_join_url: Option<Url>,
    /// The GitHub repository to fetch releases from if an installation is necessary.
    ///
    /// Defaults to "penumbra-zone/penumbra"
    #[clap(long, default_value = "penumbra-zone/penumbra")]
    repository_name: String,
    /// Disable setting up a fullnode installation.
    #[clap(long)]
    client_only: bool,
    /// By default, penv will join an existing network as specified by the [`CreateCmd::pd_join_url`].
    ///
    /// By setting this flag, penv will generate a new dev network instead.
    ///
    /// Not used if `client_only` is set.
    #[clap(long)]
    generate_network: bool,
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
    /// The GitHub repository to fetch releases from.
    ///
    /// Defaults to "penumbra-zone/penumbra"
    #[clap(long, default_value = "penumbra-zone/penumbra")]
    repository_name: String,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct InfoCmd {
    /// The alias of the Penumbra environment to print info about.
    #[clap(display_order = 100)]
    environment_alias: String,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct ResetCmd {
    /// The alias of the Penumbra environment to be reset.
    #[clap(display_order = 100)]
    environment_alias: String,
    /// Disable resetting pcli and pclientd state.
    #[clap(long)]
    leave_client_state: bool,
    /// Disable resetting pd state.
    #[clap(long)]
    leave_node_state: bool,
}

impl ManageCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        match self {
            ManageCmd {
                subcmd:
                    ManageTopSubCmd::Create(CreateCmd {
                        environment_alias,
                        penumbra_version,
                        pd_join_url,
                        grpc_url,
                        repository_name,
                        client_only,
                        generate_network,
                    }),
            } => {
                let pd_join_url = match pd_join_url {
                    Some(url) => url.clone(),
                    None => {
                        let mut grpc_url = grpc_url.clone();
                        grpc_url.set_port(Some(26657)).unwrap();
                        grpc_url.set_scheme("http").unwrap();
                        grpc_url
                    }
                };

                let mut penv = Penv::new_from_repository(repository_name.clone(), home.clone())?;

                let env = penv.create_environment(
                    environment_alias.clone(),
                    penumbra_version.clone(),
                    grpc_url.clone(),
                    pd_join_url.clone(),
                    repository_name.clone(),
                    client_only.clone(),
                    generate_network.clone(),
                )?;

                match *env {
                    Environment::BinaryEnvironment(ref env) => {
                        println!(
                            "created environment {} with pinned version {}",
                            environment_alias, env.pinned_version
                        );
                    }
                    Environment::CheckoutEnvironment(ref env) => {
                        println!(
                            "created environment {} at {} pointing to git checkout {}",
                            environment_alias,
                            env.path(),
                            env.git_checkout
                        );
                    }
                }

                Ok(())
            }
            ManageCmd {
                subcmd: ManageTopSubCmd::Delete(DeleteCmd { environment_alias }),
            } => {
                let mut penv = Penv::new(home.clone())?;

                penv.delete_environment(environment_alias.clone())?;

                Ok(())
            }
            ManageCmd {
                subcmd: ManageTopSubCmd::Info(InfoCmd { environment_alias }),
            } => {
                let penv = Penv::new(home.clone())?;

                let info = penv.environment_info(environment_alias.clone())?;

                println!("{}", info);

                Ok(())
            }
            ManageCmd {
                subcmd: ManageTopSubCmd::List(ListCmd { detailed }),
            } => {
                let penv = Penv::new(home.clone())?;

                let environments = penv.environments()?;
                let active_environment = penv.active_environment.clone();

                println!("Environments:");
                for environment in environments.iter() {
                    if *detailed {
                        if active_environment
                            .clone()
                            .is_some_and(|e| e.metadata().alias == environment.metadata().alias)
                        {
                            print!("{}", format!("{}Active: true\n\n", environment).green());
                        } else {
                            print!("{}", format!("{}Active: false\n\n", environment).red());
                        }
                    } else {
                        if active_environment
                            .clone()
                            .is_some_and(|e| e.metadata().alias == environment.metadata().alias)
                        {
                            print!(
                                "{}",
                                format!("{} (active)\n\n", environment.metadata().alias).green()
                            );
                        } else {
                            print!("{}", format!("{}\n\n", environment.metadata().alias).red());
                        }
                    }
                }

                Ok(())
            }
            ManageCmd {
                subcmd:
                    ManageTopSubCmd::Reset(ResetCmd {
                        environment_alias,
                        leave_client_state,
                        leave_node_state,
                    }),
            } => {
                let mut penv = Penv::new(home.clone())?;

                penv.reset_environment(
                    environment_alias.clone(),
                    leave_client_state.clone(),
                    leave_node_state.clone(),
                )?;

                Ok(())
            }
            ManageCmd {
                subcmd:
                    ManageTopSubCmd::Upgrade(UpgradeCmd {
                        environment_alias,
                        repository_name,
                    }),
            } => {
                let mut penv = Penv::new_from_repository(repository_name.clone(), home.clone())?;

                let environment = penv.environments.get_environment(&environment_alias);
                if environment.is_none() {
                    return Err(anyhow!(
                        "Environment with alias {} does not exist",
                        environment_alias
                    ));
                }

                let environment = environment.unwrap();

                let (penumbra_version, pinned_version) = match *environment {
                    Environment::BinaryEnvironment(ref env) => (
                        RepoOrVersionReq::VersionReqOrLatest(env.version_requirement.clone()),
                        env.pinned_version.clone(),
                    ),
                    Environment::CheckoutEnvironment(ref _env) => {
                        panic!("checkout environments are not supported for upgrades")
                    }
                };

                // Find the best matching version
                let cache = &penv.cache;
                let matching_installed_version = match cache.find_best_match(&penumbra_version) {
                    Some(installed_version) => installed_version,
                    None => {
                        // TODO: allow auto-installing here
                        return Err(anyhow!(
                            "No installed version found for version requirement {}",
                            penumbra_version
                        ));
                    }
                };

                match *matching_installed_version {
                    InstalledRelease::GitCheckout(ref _release) => {
                        unreachable!("git checkout environments are not supported for upgrades")
                    }
                    InstalledRelease::Binary(ref matching_installed_version) => {
                        if matching_installed_version.version == pinned_version {
                            println!(
                                "Environment {}'s pinned version {} is the latest installed version matching version requirement {}",
                                environment_alias, pinned_version, penumbra_version
                            );
                            return Ok(());
                        }

                        println!(
                            "Updating environment {}'s pinned version from {} to the latest installed version {}",
                            environment_alias, pinned_version, matching_installed_version.version
                        );
                        penv.replace_version(
                            environment_alias.clone(),
                            matching_installed_version.version.clone(),
                        )?;
                        penv.persist()?;
                    }
                }
                Ok(())
            }
            &ManageCmd {
                subcmd: ManageTopSubCmd::Rename(_),
            } => unimplemented!(),
        }
    }
}
