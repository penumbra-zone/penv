#![deny(clippy::unwrap_used)]

use std::fs;

use anyhow::{Context as _, Result};
use clap::Parser;

use penv::Penv;
use penv::{command::Command, opt::Opt};

#[tokio::main]
async fn main() -> Result<()> {
    let mut opt = Opt::parse();

    opt.init_tracing();

    // Ensure that the penv home dir exists, in case this is a cold start
    fs::create_dir_all(&opt.home)
        .with_context(|| format!("Failed to create home directory {}", opt.home))?;

    let cmd = &opt.cmd;
    match cmd {
        Command::Install(install_cmd) => install_cmd.exec(opt.home).await?,
        Command::Cache(cache_cmd) => cache_cmd.exec(opt.home).await?,
        Command::Manage(manage_cmd) => {
            manage_cmd.exec(opt.home).await?;
            ()
        }
        Command::Use(use_cmd) => use_cmd.exec(opt.home).await?,
        Command::Hook(hook_cmd) => hook_cmd.exec(opt.home).await?,
        Command::Env(env_cmd) => env_cmd.exec(opt.home).await?,
        Command::Which(which_cmd) => which_cmd.exec(opt.home).await?,
        Command::UnsafeResetAll => {
            // rm the home directory
            println!("removing directory {}", opt.home);
            std::fs::remove_dir_all(&opt.home)?;
        }
        Command::Deactivate => {
            println!("deactivating active environment...");
            let mut penv = Penv::new(opt.home.clone())?;
            penv.active_environment = None;
            // Unset the symlink
            let link = penv.home_dir.join("bin");
            let link_metadata = fs::metadata(link.clone());
            tracing::debug!("link_metadata: {:?}", link_metadata);
            if let Ok(link_metadata) = link_metadata {
                if link_metadata.is_symlink() || link_metadata.is_file() {
                    tracing::debug!("removing symlink");
                    fs::remove_file(link)?;
                } else if link_metadata.is_dir() {
                    tracing::debug!("removing symlink");
                    fs::remove_dir_all(link)?;
                }
            } else {
                tracing::debug!("symlink path {} does not exist", link);
            }
            penv.persist()?;
            println!("deactivated");
        }
    }

    Ok(())
}
