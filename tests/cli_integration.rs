//! Integration tests to exercise the penv CLI actions.
//! Will perform actual network calls against remote hosts, so run judiciously.
#![cfg(feature = "network-integration")]

use assert_cmd::Command;
use tempfile::tempdir;

/// The version of the CLI programs to fetch from github.
const PENUMBRA_VERSION: &str = "0.80";

/// For better or worse, the `penv manage create <foo>` command fails, unless
/// `penv manage install <version>` was run first. When we add auto-installation
/// logic we can update this test and expect it to pass.
#[tokio::test]
async fn creating_environment_without_prior_installation_fails() -> anyhow::Result<()> {
    let penv_home = tempdir().unwrap();
    let mut create_cmd = Command::cargo_bin("penv").unwrap();
    create_cmd.args([
        "--home",
        penv_home.path().to_str().unwrap(),
        "manage",
        "create",
        "testnet",
        "--grpc-url",
        "https://testnet.plinfra.net",
        "--penumbra-version",
        PENUMBRA_VERSION,
    ]);
    create_cmd.assert().failure();
    Ok(())
}

#[tokio::test]
/// Confirm that we can create a fresh environment, via:
///
///   penv --home <tempdir> manage create testnet --grpc-url https://testnet.plinfra.net
///   --penumbra-version 0.80
///
/// That command should work just fine. First, we'll need to install the necessary deps.
async fn create_testnet_environment() -> anyhow::Result<()> {
    let penv_home = tempdir().unwrap();
    let mut install_cmd = Command::cargo_bin("penv").unwrap();
    install_cmd.args([
        "--home",
        penv_home.path().to_str().unwrap(),
        "install",
        PENUMBRA_VERSION,
    ]);
    install_cmd.assert().success();
    let mut create_cmd = Command::cargo_bin("penv").unwrap();
    create_cmd.args([
        "--home",
        penv_home.path().to_str().unwrap(),
        "manage",
        "create",
        "testnet",
        "--grpc-url",
        "https://testnet.plinfra.net",
        "--penumbra-version",
        PENUMBRA_VERSION,
    ]);
    create_cmd.assert().success();
    Ok(())
}
