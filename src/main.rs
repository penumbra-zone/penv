#![deny(clippy::unwrap_used)]

use std::fs;

use anyhow::{Context as _, Result};
use clap::Parser;

use pvm::{command::Command, opt::Opt};

#[tokio::main]
async fn main() -> Result<()> {
    let mut opt = Opt::parse();

    opt.init_tracing();

    // Ensure that the pvm home dir exists, in case this is a cold start
    fs::create_dir_all(&opt.home)
        .with_context(|| format!("Failed to create home directory {}", opt.home))?;

    let cmd = &opt.cmd;
    match cmd {
        Command::Install(install_cmd) => install_cmd.exec(opt.home).await?,
        Command::Cache(cache_cmd) => cache_cmd.exec(opt.home).await?,
        _ => unimplemented!("not implemented yet :("),
    }

    Ok(())
}
