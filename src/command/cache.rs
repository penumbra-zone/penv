use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::value_parser;
// TODO: better handle colorized text with flags
use colored::Colorize;

use crate::pvm::environment::EnvironmentTrait;
use crate::pvm::release::{RepoOrVersion, RepoOrVersionReq};

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
    /// List all versions available from the repository.
    #[clap(display_order = 300)]
    Available(AvailableCmd),
    /// Completely reset the cache, removing all installed versions.
    #[clap(display_order = 400)]
    Reset,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct ListCmd {
    /// Only list versions matching the given semver version requirement.
    required_version: Option<RepoOrVersionReq>,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct AvailableCmd {
    /// Only list versions matching the given semver version requirement.
    required_version: Option<RepoOrVersionReq>,
}

#[derive(Debug, Clone, clap::Parser)]
pub struct DeleteCmd {
    /// The cached installation to delete.
    #[clap(value_parser = value_parser!(RepoOrVersion))]
    version: RepoOrVersion,
}

impl CacheCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        match self {
            CacheCmd {
                subcmd: CacheTopSubCmd::List(ListCmd { required_version }),
            } => {
                let cache = crate::pvm::cache::cache::Cache::new(home)?;
                let versions = cache.list_installed(required_version.as_ref())?;
                for version in versions {
                    println!("{}", version);
                }
                Ok(())
            }
            CacheCmd {
                subcmd: CacheTopSubCmd::Delete(DeleteCmd { version }),
            } => {
                // don't allow deletion if environment uses this version
                let mut pvm = crate::pvm::Pvm::new(home.clone())?;
                if let Some(env) = pvm
                    .environments
                    .iter()
                    .find(|e| (**e).satisfied_by_version(version))
                {
                    return Err(anyhow::anyhow!(
                        "Cannot delete version {} because it is pinned by environment {}",
                        version,
                        env.metadata().alias
                    ));
                }

                let installed_version = pvm.cache.get_installed_release(version);

                match installed_version {
                    Some(installed_version) => {
                        // TODO: cloning here is dumb and defeats the point of taking ownership
                        pvm.cache.delete(installed_version.clone())?;
                        pvm.cache.persist()?;
                        Ok(())
                    }
                    None => return Err(anyhow!("Version {} is not installed", version)),
                }
            }
            CacheCmd {
                subcmd: CacheTopSubCmd::Available(AvailableCmd { required_version }),
            } => {
                let pvm = crate::pvm::Pvm::new(home.clone())?;
                let releases = pvm.list_available(required_version.as_ref()).await?;
                for (release, installed) in releases {
                    if installed {
                        println!("{}", release.version.to_string().green());
                    } else {
                        println!("{}", release.version.to_string().red());
                    }
                }
                Ok(())
            }
            CacheCmd {
                subcmd: CacheTopSubCmd::Reset,
            } => {
                // Wipe the existing directory.
                let config_path = home.join("cache.toml");
                if config_path.exists() {
                    std::fs::remove_file(&config_path)?;
                }

                // Re-instantiate and persist the cache.
                let cache = crate::pvm::cache::cache::Cache::new(home)?;
                cache.persist()?;

                Ok(())
            }
        }
    }
}
