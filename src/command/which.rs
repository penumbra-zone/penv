use anyhow::Result;
use camino::Utf8PathBuf;

use crate::pvm::Pvm;

#[derive(Debug, clap::Parser)]
pub struct WhichCmd {
    /// Display additional information about the configured environment.
    #[clap(long)]
    detailed: bool,
}

impl WhichCmd {
    pub async fn exec(&self, home: Utf8PathBuf) -> Result<()> {
        let detailed = &self.detailed;
        let pvm = Pvm::new(home.clone())?;
        let active_environment = pvm.active_environment.clone();

        match active_environment {
            Some(env) => {
                if *detailed {
                    println!("{}", env);
                } else {
                    println!("{}", env.alias);
                }
            }
            None => {
                println!("no active environment set");
            }
        }

        Ok(())
    }
}
