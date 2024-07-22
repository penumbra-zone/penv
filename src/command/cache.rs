use anyhow::Result;
use camino::Utf8PathBuf;
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
    /// Completely reset the cache, removing all installed versions.
    #[clap(display_order = 300)]
    Reset,
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

impl CacheCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        match self {
            CacheCmd {
                subcmd: CacheTopSubCmd::List(ListCmd { required_version }),
            } => {
                let cache = crate::pvm::cache::cache::Cache::new(home)?;
                let versions = cache.list(required_version.as_ref())?;
                for version in versions {
                    println!("{}", version);
                }
                Ok(())
            }
            CacheCmd {
                subcmd: CacheTopSubCmd::Delete(DeleteCmd { version }),
            } => {
                let cache = crate::pvm::cache::cache::Cache::new(home);
                // cache.delete(version).await?;
                Ok(())
            }
            CacheCmd {
                subcmd: CacheTopSubCmd::Reset,
            } => {
                // Wipe the existing directory.
                if home.exists() {
                    std::fs::remove_dir_all(&home)?;
                }

                // Re-instantiate and persist the cache.
                let cache = crate::pvm::cache::cache::Cache::new(home)?;
                cache.persist()?;

                Ok(())
            }
        }
    }
}
