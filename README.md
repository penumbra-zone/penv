# pvm, the Penumbra Version Manager

## Description

Though a lot of care has been taken to ensure [Penumbra](https://penumbra.zone) is
at the forefront of usability and developer experience, it is an unavoidable fact
that running Penumbra requires synchronizing various versions of software dependencies.

For example, between Penumbra major releases, node operators will typically need to migrate
their state data to be compatible with new features and bug fixes. APIs also may change
between versions. For these reasons, using outdated versions of the Penumbra client software
won't work against updated nodes.

Additionally, node operators or developers may wish to test migration processes. `pvm` makes this
easier by allowing users to set up isolated Penumbra environments, associating a particular version
of software with configuration and data necessary to run Penumbra.

## Installation

`pvm` works by setting up pre-command execution hooks in your shell to set the appropriate environment.

Each shell will require its own installation process.

### zsh

Add the following line at the end of the `~/.zshrc` file:

```shell
eval "$(pvm hook zsh)"
```

## Demo

After installing the hook in your shell, you can begin using `pvm`.

### Listing available versions

You can check which versions are available to install:

```console
$ pvm cache available 0.79
fetching available releases from https://api.github.com/repos/penumbra-zone/penumbra/releases
0.79.2
0.79.1
0.79.0
```

This command takes a semver version requirement to filter available versions. Installed versions will
be displayed in <span style="color:green">_green_</span> and uninstalled versions in <span style="color:red">_red_</span>.

### Installing Penumbra versions

First, install a version of the Penumbra software using `pvm cache install VERSION`.
The version is specified as a semver version requirement; the latest version on the
repository matching the version requirement will be installed to pvm's installation
cache.

```console
$ pvm install '0.79.0'
installing ^0.79.0
fetching available releases from https://api.github.com/repos/penumbra-zone/penumbra/releases
downloading latest matching release: 0.79.2
downloading shasum from https://github.com/penumbra-zone/penumbra/releases/download/v0.79.2/pcli-aarch64-apple-darwin.tar.gz.sha256
downloading shasum from https://github.com/penumbra-zone/penumbra/releases/download/v0.79.2/pclientd-aarch64-apple-darwin.tar.gz.sha256
downloading shasum from https://github.com/penumbra-zone/penumbra/releases/download/v0.79.2/pd-aarch64-apple-darwin.tar.gz.sha256
downloading archive from https://github.com/penumbra-zone/penumbra/releases/download/v0.79.2/pcli-aarch64-apple-darwin.tar.gz
downloading archive from https://github.com/penumbra-zone/penumbra/releases/download/v0.79.2/pclientd-aarch64-apple-darwin.tar.gz
downloading archive from https://github.com/penumbra-zone/penumbra/releases/download/v0.79.2/pd-aarch64-apple-darwin.tar.gz
  [00:00:05] [########################################] 97.97MiB/97.97MiB (0s)
  [00:00:07] [########################################] 94.65MiB/94.65MiB (0s)
  [00:00:08] [########################################] 117.86MiB/117.86MiB (0s)
installing latest matching release: 0.79.2
```

### Listing installed versions

You can verify which versions have been installed to the cache:

```console
$ pvm cache list
0.79.2
```

This command also takes an optional semver version requirement to filter installed versions.

### Creating an environment

Now that you've installed a version of the Penumbra software, you can
configure a new environment using that version of the software.

The basic format of the command is `pvm manage create ALIAS VERSION_REQUIREMENT GRPC_URL` followed by optional flags.

```console
$ pvm manage create v0.79.x-localhost 0.79 http://localhost:26657 --client-only
created environment v0.79.x-localhost with pinned version 0.79.2
```

This will create a new environment with the alias `v0.79.x-localhost` using the latest installed version matching the semver requirement `^0.79`. Additionally, the `--client-only` flag means
only `pcli`/`pclientd` binaries will be installed and have configurations initialized; leave this
flag off if you also want `pd` node software to be configured in the environment.

### Listing environments

To view the configured environments and their details:

```console
$ pvm manage list --detailed
EEnvironments:
Alias: v0.79.x-localhost
GRPC URL: http://localhost:26657/
Version Requirement: ^0.79
Pinned Version: 0.79.2
Root Directory: /Users/user/Library/Application Support/zone.penumbra.pvm/environments/v0.79.x-localhost
Include Node: false
Active: false
```

The active environment will
be displayed in <span style="color:green">_green_</span> and inactive environments in <span style="color:red">_red_</span>.

### Activating environments

You can activate a configured environment:

```console
$ pvm which
no active environment set
```

```console
$ pvm use v0.79.x-localhost
activating v0.79.x-localhost...
activated
```

```console
$ pvm which
v0.79.x-localhost

$ pvm which --detailed
Alias: v0.79.x-localhost
GRPC URL: http://localhost:26657/
Version Requirement: ^0.79
Pinned Version: 0.79.2
Root Directory: /Users/user/Library/Application Support/zone.penumbra.pvm/environments/v0.79.x-localhost
Include Node: false
```

Additionally, since the hook has been installed to your shell, necessary environment variables will be set:

```console
$ echo $PVM_ACTIVE_ENVIRONMENT
v0.79.x-localhost

$ echo $PENUMBRA_PCLI_HOME
/Users/user/Library/Application Support/zone.penumbra.pvm/environments/v0.79.x-mainnet/pcli
```

And your `PATH` will be updated to point to the correct binary versions:

```console
$ pcli --version
pcli 0.79.2
```

## Security

If you believe you've found a security-related issue with pvm,
please disclose responsibly by contacting the Penumbra Labs team at
security@penumbralabs.xyz.

## License

By contributing to pvm you agree that your contributions will be licensed
under the terms of both the [LICENSE-Apache-2.0](LICENSE-Apache-2.0) and the
[LICENSE-MIT](LICENSE-MIT) files in the root of this source tree.

If you're using pvm you are free to choose one of the provided licenses:

`SPDX-License-Identifier: MIT OR Apache-2.0`
