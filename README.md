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
