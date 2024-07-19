#![deny(clippy::unwrap_used)]
#![allow(clippy::clone_on_copy)]

use camino::Utf8PathBuf;
use directories::ProjectDirs;

pub mod command;
pub mod opt;

pub fn default_home() -> Utf8PathBuf {
    let path = ProjectDirs::from("zone", "penumbra", "pvm")
        .expect("Failed to get platform data dir")
        .data_dir()
        .to_path_buf();
    Utf8PathBuf::from_path_buf(path).expect("Platform default data dir was not UTF-8")
}
