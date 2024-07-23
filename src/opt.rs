use crate::default_home as default_pvm_home;
use camino::Utf8PathBuf;
use clap::Parser;
// TODO: having to compile the world of penumnbra just to get these
// is annoying, maybe we can do something with a feature flag or just hardcode them
use pcli::default_home as default_pcli_home;
use pclientd::default_home as default_pclientd_home;
use std::io::IsTerminal as _;
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::command::Command;

#[derive(Debug, Parser)]
#[clap(
    name = "pvm",
    about = "The Penumbra version manager command-line interface.",
    version
)]
pub struct Opt {
    #[clap(subcommand)]
    pub cmd: Command,
    /// The home directory used to store pvm-related configuration and cache data.
    #[clap(long, default_value_t = default_pvm_home(), env = "PENUMBRA_PVM_HOME")]
    pub home: Utf8PathBuf,
    /// The home directory used to store pcli-related configuration and data.
    #[clap(long, default_value_t = default_pcli_home(), env = "PENUMBRA_PCLI_HOME")]
    pub pcli_home: Utf8PathBuf,
    /// The home directory used to store pclientd-related state and data.
    #[clap(long, default_value_t = default_pclientd_home(), env = "PENUMBRA_PCLIENTD_HOME")]
    pub pclientd_home: Utf8PathBuf,
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
