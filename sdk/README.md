# ADM SDK

[![License](https://img.shields.io/github/license/amazingdatamachine/adm.svg)](../LICENSE)
[![standard-readme compliant](https://img.shields.io/badge/standard--readme-OK-green.svg)](https://github.com/RichardLitt/standard-readme)

> The Amazing Data Machine (ADM) SDK

## Table of Contents

- [Table of Contents](#table-of-contents)
- [Background](#background)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)

## Background

The ADM CLI is a tool for managing your account and data machines.

- _Object store machines_:
  These are key-value stores that allow you to push and retrieve data in a familiar S3-like fashion.
  Object stores support byte range requests and advanced queries based on key prefix, delimiter, offset, and
  limit.
- _Accumulator machines_:
  An accumulator is a [Merkle Mountain Range (MMR)](https://docs.grin.mw/wiki/chain-state/merkle-mountain-range/)-based
  verifiable anchoring system for state updates.
  You can push values up to 500KiB and retrieve them by index.
  Accumulators support querying for state root, MMR peaks, and total leaf count.

Read more about data machines [here](../README.md).

The SDK consists of the following crates:

- [`adm_provider`](../provider): A chain and object provider for the ADM.
- [`adm_signer`](../signer): A transaction signer for the ADM.
  This crate has a built-in [wallet](../signer/src/wallet.rs) signer implementation that relies on a local private key
  to sign messages.
- [`adm_sdk`](.): The top-level user interface for managing ADM object storage and state accumulators.

The `adm` crates haven't been published yet, but you can read the Cargo docs by building them locally from the repo
root.

```shell
# Build cargo docs and open in your default browser
make doc
```

### Prerequisites

All data is signed onchain as transactions, so you'll need to set up an account (ECDSA, secp256k1) to use the ADM
network.
For example, any EVM-compatible wallet will work, or you can run
the [`account_deposit.rs`](./examples/account_deposit.rs) example to create a private key for you.

Then, make sure your account is funded with FIL, so you can pay to execute a transaction (you can use the
faucet [here](https://faucet.calibnet.chainsafe-fil.io/funds.html)).
Follow the [examples](./examples) to get up and running.

## Usage

Checkout the SDK [examples](./examples).
The `adm` crates haven't been published yet, but you can use `adm_sdk` as a git dependencies.

```toml
[dependencies]
adm_sdk = { git = "https://github.com/textileio/basin.git" }
```

> [!NOTE]
> To use this crate in another crate, include this patch
> for [`merkle-tree-rs`](https://github.com/consensus-shipyard/merkle-tree-rs) in your `Cargo.toml`.
> ```toml
> [patch.crates-io]
> # Contains some API changes that the upstream has not merged.
> merkle-tree-rs = { git = "https://github.com/consensus-shipyard/merkle-tree-rs.git", branch = "dev" }
> ```

This issue will be fixed when the `adm` crates get published soon.

## Contributing

PRs accepted.

Small note: If editing the README, please conform to
the [standard-readme](https://github.com/RichardLitt/standard-readme) specification.

## License

MIT OR Apache-2.0, © 2024 ADM Contributors
