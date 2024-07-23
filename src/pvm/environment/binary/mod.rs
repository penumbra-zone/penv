pub(crate) trait Binary {
    fn path(&self) -> Utf8PathBuf;
    fn initialize(&self) -> Result<()>;
}

mod pcli;
mod pclientd;
mod pd;

use anyhow::Result;
use camino::Utf8PathBuf;
pub(crate) use pcli::*;
pub(crate) use pclientd::*;
pub(crate) use pd::*;
