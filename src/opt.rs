use crate::default_home as default_penv_home;
use camino::Utf8PathBuf;
use clap::Parser;
use std::io::IsTerminal as _;
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::command::Command;

#[derive(Debug, Parser)]
#[clap(
    name = "penv",
    about = "The Penumbra environment manager command-line interface.",
    version
)]
pub struct Opt {
    #[clap(subcommand)]
    pub cmd: Command,
    /// The home directory used to store penv-related configuration and cache data.
    #[clap(long, default_value_t = default_penv_home(), env = "PENUMBRA_PENV_HOME")]
    pub home: Utf8PathBuf,
    /// Override the GRPC URL that will be used to connect to a fullnode.
    ///
    /// By default, this URL is provided by pcli's config. See `pcli init` for more information.
    #[clap(long, parse(try_from_str = Url::parse))]
    pub grpc_url: Option<Url>,
}

impl Opt {
    pub fn init_tracing(&mut self) {
        tracing_subscriber::fmt()
            .with_ansi(std::io::stdout().is_terminal())
            .with_env_filter(EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    }
}
