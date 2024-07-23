use anyhow::Result;
use camino::Utf8PathBuf;

use crate::pvm::Pvm;

#[derive(Debug, clap::Parser)]
pub struct UseCmd {
    /// The alias of the Penumbra environment to be activated.
    environment_alias: String,
}

impl UseCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let environment_alias = &self.environment_alias;
        println!("activating {}...", environment_alias);
        let mut pvm = Pvm::new(home.clone())?;
        pvm.activate(environment_alias.to_string())?;
        pvm.persist()?;

        println!("activated");

        Ok(())
    }
}
