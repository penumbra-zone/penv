#[derive(Debug, clap::Parser)]
pub struct EnvCmd {
    /// Which shell environment to print configuration for.
    shell: String,
}
