#[derive(Debug, clap::Parser)]
pub struct WhichCmd {
    /// Display additional information about the configured environment.
    #[clap(long)]
    detailed: bool,
}
