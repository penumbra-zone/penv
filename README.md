# penv

An environment manager for [Penumbra].

## Motivation

The `penv` tool is intended as a utility _for developers_ who work on multiple
versions of the Penumbra protocol. For instance, [Penumbra Labs] runs a [testnet]
to evaluate next-generation features that may or may not be adopted by mainnet.
Sometimes there's protocol incompatibility between testnet and mainnet,
so specific software versions must be used for each network. `penv` helps to organize those
software version and chain associations.

Additionally, developers may wish to configure multiple wallets per chain, or use multiple RPC
endpoints for their `pcli` configuration. `penv` allows switching between these environments
quickly and easily.

Beyond managing client software such as `pcli` and `pclientd`, `penv` can also manage
local [devnet]s with `pd` for use in complicated scenarios like migration testing.

**If you just want to use `pcli` on the command-line, you should not use this tool.**
Instead, consult the [project documentation for using `pcli`](https://guide.penumbra.zone/pcli).

## Installation

`penv` works by setting up pre-command execution hooks in your shell to set the appropriate environment.
First, make sure `penv` is on your path by building it locally:

```
cargo install --path .
```

Then, each shell will require its own installation process.

### zsh

Add the following line at the end of the `~/.zshrc` file:

```shell
eval "$(penv hook zsh)"
```

### bash

Add the following line at the end of the `~/.bashrc` file:

```shell
eval "$(penv hook bash)"
```

## Usage

After installing the hook in your shell, you can begin using `penv`.

### Listing available versions

You can check which versions are available to install:

```console
$ penv cache available
fetching available releases from https://api.github.com/repos/penumbra-zone/penumbra/releases
2.0.0-alpha.11
1.5.2
2.0.0-alpha.10
1.5.1
1.5.0
```

This command takes a semver version requirement to filter available versions. Installed versions will
be displayed in <span style="color:green">_green_</span> and uninstalled versions in <span style="color:red">_red_</span>.

### Installing Penumbra versions

First, install a version of the Penumbra software using `penv cache install VERSION`.
The version is specified as a semver version requirement; the latest version on the
repository matching the version requirement will be installed to penv's installation
cache.

```console
$ penv install 1.5.2
installing ^1.5.2
fetching available releases from https://api.github.com/repos/penumbra-zone/penumbra/releases
downloading latest matching release: 1.5.2
downloading shasum from https://github.com/penumbra-zone/penumbra/releases/download/v1.5.2/pcli-x86_64-unknown-linux-gnu.tar.gz.sha256
downloading shasum from https://github.com/penumbra-zone/penumbra/releases/download/v1.5.2/pclientd-x86_64-unknown-linux-gnu.tar.gz.sha256
downloading shasum from https://github.com/penumbra-zone/penumbra/releases/download/v1.5.2/pd-x86_64-unknown-linux-gnu.tar.gz.sha256
downloading archive from https://github.com/penumbra-zone/penumbra/releases/download/v1.5.2/pcli-x86_64-unknown-linux-gnu.tar.gz
downloading archive from https://github.com/penumbra-zone/penumbra/releases/download/v1.5.2/pclientd-x86_64-unknown-linux-gnu.tar.gz
downloading archive from https://github.com/penumbra-zone/penumbra/releases/download/v1.5.2/pd-x86_64-unknown-linux-gnu.tar.gz
  [00:00:02] [########################################] 97.63MiB/97.63MiB (0s)
  [00:00:00] [########################################] 12.07MiB/12.07MiB (0s)
  [00:00:01] [########################################] 50.10MiB/50.10MiB (0s)
installing latest matching release: 1.5.2
```

### Listing installed versions

You can verify which versions have been installed to the cache:

```console
$ penv cache list
0.80.4
0.80.5
0.80.7
0.80.9
0.80.10
0.80.11
0.81.0
1.5.0
1.5.1
1.5.2
2.0.0-alpha.10
2.0.0-alpha.11
```

This command also takes an optional semver version requirement to filter installed versions.

### Creating an environment

Now that you've installed a version of the Penumbra software, you can
configure a new environment using that version of the software.

The basic format of the command is `penv manage create ALIAS VERSION_REQUIREMENT GRPC_URL` followed by optional flags.

```console
$ penv manage create v0.79.x-localhost 0.79 http://localhost:26657 --client-only
created environment v0.79.x-localhost with pinned version 0.79.2
```

This will create a new environment with the alias `v0.79.x-localhost` using the latest installed version matching the semver requirement `^0.79`. Additionally, the `--client-only` flag means
only `pcli`/`pclientd` binaries will be installed and have configurations initialized; leave this
flag off if you also want `pd` node software to be configured in the environment.

### Listing environments

To view the configured environments and their details:

```console
$ penv manage list --detailed
Environments:
Alias: v0.79.x-localhost
GRPC URL: http://localhost:26657/
Version Requirement: ^0.79
Pinned Version: 0.79.2
Root Directory: /Users/user/Library/Application Support/zone.penumbra.penv/environments/v0.79.x-localhost
Include Node: false
Active: false
```

The active environment will
be displayed in <span style="color:green">_green_</span> and inactive environments in <span style="color:red">_red_</span>.

### Activating environments

You can activate a configured environment:

```console
$ penv which
no active environment set
```

```console
$ penv use v0.79.x-localhost
activating v0.79.x-localhost...
activated
```

```console
$ penv which
v0.79.x-localhost

$ penv which --detailed
Alias: v0.79.x-localhost
GRPC URL: http://localhost:26657/
Version Requirement: ^0.79
Pinned Version: 0.79.2
Root Directory: /Users/user/Library/Application Support/zone.penumbra.penv/environments/v0.79.x-localhost
Include Node: false
```

Additionally, since the hook has been installed to your shell, necessary environment variables will be set:

```console
$ echo $PENV_ACTIVE_ENVIRONMENT
v0.79.x-localhost

$ echo $PENUMBRA_PCLI_HOME
/Users/user/Library/Application Support/zone.penumbra.penv/environments/v0.79.x-localhost/pcli
```

And your `PATH` will be updated to point to the correct binary versions:

```console
$ pcli --version
pcli 0.79.2
```

## Upgrading environments

When a new point release is made, you can update a specific environment by running:

```
penv install 0.79
penv manage upgrade <environment>
```

There's not yet auto-install logic, so simply running `penv manage upgrade <environment>` will only symlink
the binaries for the most recent release already installed locally. The `penv install <version>` command
will fetch the latest release.

The `upgrade` logic is not smart enough to handle running migrations e.g. for `pd`.

## Environment Variables

`penv` sets various environment variables.

For example, to run `cometbft` after activating an environment, you can use the `COMETBFT_HOME` environment variable:

```console
$ cometbft start --home $COMETBFT_HOME
```

The entire list of environment variables is:

```
PENV_ACTIVE_ENVIRONMENT
PENUMBRA_PCLI_HOME
PENUMBRA_PCLIENTD_HOME
PENUMBRA_PD_HOME
PENUMBRA_PD_COMETBFT_PROXY_URL
PENUMBRA_PD_JOIN_URL
COMETBFT_HOME
```

## Working With Git Checkouts

You can also use `penv` to create an environment based on a git checkout.

For example, to create a dev environment with a generated local devnet based on the `penumbra-zone/penumbra` GitHub repository:

```console
$ penv install 'git@github.com:penumbra-zone/penumbra.git'

installing git@github.com:penumbra-zone/penumbra.git
installing latest matching release: git@github.com:penumbra-zone/penumbra.git (git@github.com:penumbra-zone/penumbra.git)
cloning repo git@github.com:penumbra-zone/penumbra.git to /Users/user/Library/Application Support/zone.penumbra.penv/checkouts/91734fec0f7dc59357c94a82abc0eb927ae4f07a151d10f280a189623c3af9e8
fetch...
Checking out into "/Users/user/Library/Application Support/zone.penumbra.penv/checkouts/91734fec0f7dc59357c94a82abc0eb927ae4f07a151d10f280a189623c3af9e8" ...

$ penv manage create main_repo-devnet 'git@github.com:penumbra-zone/penumbra.git' http://localhost:8080 --generate-network

created environment main_repo-devnet at /Users/user/Library/Application Support/zone.penumbra.penv/environments/main_repo-devnet pointing to git checkout git@github.com:penumbra-zone/penumbra.git (git@github.com:penumbra-zone/penumbra.git)
```

When you activate the environment, your shell will be populated with `pcli`/`pclientd`/`pd` wrappers that build the corresponding binary from the git repository.

```console
$ which pcli

/Users/user/Library/Application Support/zone.penumbra.penv/bin/pcli

$ ls -la /Users/user/Library/Application\ Support/zone.penumbra.penv/

total 32
drwxr-xr-x   8 user staff   256 Jul 25 16:26 .
drwx------+ 99 user staff  3168 Jul 25 16:19 ..
lrwxr-xr-x   1 user staff    96 Jul 25 16:26 bin -> /Users/user/Library/Application Support/zone.penumbra.penv/environments/main_repo-devnet/bin
-rw-r--r--   1 user staff  5314 Jul 25 16:26 cache.toml
drwxr-xr-x   3 user staff    96 Jul 25 16:20 checkouts
drwxr-xr-x   4 user staff   128 Jul 25 16:21 environments
-rw-r--r--   1 user staff  6671 Jul 25 16:26 penv.toml
drwxr-xr-x   3 user staff    96 Jul 25 16:20 versions


$ cat /Users/user/Library/Application\ Support/zone.penumbra.penv/environments/main_repo-devnet/bin/pcli

exec cargo run --manifest-path="/Users/user/Library/Application Support/zone.penumbra.penv/environments/main_repo-devnet/checkout/Cargo.toml" --release --bin pcli -- "$@"
```

The working git repository will be placed in the `checkout` subdirectory of the relevant environment, for example: `/Users/user/Library/Application Support/zone.penumbra.penv/environments/main_repo-devnet/checkout`.

## Security

If you believe you've found a security-related issue with penv,
please disclose responsibly by contacting the Penumbra Labs team at
security@penumbralabs.xyz.

## License

By contributing to penv you agree that your contributions will be licensed
under the terms of both the [LICENSE-Apache-2.0](LICENSE-Apache-2.0) and the
[LICENSE-MIT](LICENSE-MIT) files in the root of this source tree.

If you're using penv you are free to choose one of the provided licenses:

`SPDX-License-Identifier: MIT OR Apache-2.0`

[Penumbra]: https://penumbra.zone
[Penumbra Labs]: https://penumbralabs.xyz
[testnet]: https://guide.penumbra.zone/dev/testnet
[devnet]: http://guide.penumbra.zone/dev/devnet-quickstart
[pcli]: https://guide.penumbra.zone/pcli
