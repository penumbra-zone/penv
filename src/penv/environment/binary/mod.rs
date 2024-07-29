// TODO: this crate seems like it should hold BinaryEnvironment (like the [`super::checkout`] module) but it doesn't
// it holds the binary definitions instead (which are used by _all_ environments),
// better organization is needed
pub(crate) trait Binary: ManagedFile {
    fn initialize(&self, configs: Option<HashMap<String, String>>) -> Result<String>;
}

pub(crate) trait ManagedFile {
    fn path(&self) -> Utf8PathBuf;
}

mod pcli;
mod pclientd;
mod pd;

use std::collections::HashMap;

use anyhow::Result;
use camino::Utf8PathBuf;
pub(crate) use pcli::*;
pub(crate) use pclientd::*;
pub(crate) use pd::*;
