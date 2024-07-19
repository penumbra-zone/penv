#[derive(Debug, clap::Parser)]
pub struct UseCmd {
    /// The alias of the Penumbra environment to be activated.
    environment_alias: String,
}
