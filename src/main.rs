#![deny(clippy::unwrap_used)]

use anyhow::Result;
use clap::Parser;

use pvm::opt::Opt;

#[tokio::main]
async fn main() -> Result<()> {
    let mut opt = Opt::parse();

    opt.init_tracing();

    Ok(())
}
