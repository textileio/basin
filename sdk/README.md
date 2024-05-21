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

The ADM SDK comes packed with account management and machine control for scalable data storage.
You can create an object store and accumulator machine, push data to them, and retrieve the data (or relevant
information about it).
The object store machine is a key-value store that allows you to spin up new stores, push, and retrieve data.
Plus, features like range requests and other filters add convenience its usage.
The accumulator machine is a hashed structure that allows
you to push data to it and retrieve the root, leaf, or count of the tree; a verifiable anchoring system.

### Prerequisites

All data is signed onchain as transactions, so you'll need to set up an account (ECDSA, secp256k1) to use the ADM
network.
For example, any EVM-compatible wallet will work, or you can run
the [`account_deposit.rs`](./examples/account_deposit.rs) example to create a private key for you.

Then, make sure your account is funded with FIL, so you can pay to execute a transaction (you can use the
faucet [here](https://faucet.calibnet.chainsafe-fil.io/funds.html)).
When you `deposit` funds from the parent (Filecoin Calibration) to the child subnet,
it will register your account on the subnet.
If you ever want to move funds back to the parent, the `withdraw` command can be used.
Note these differ from moving funds intra-subnet, which requires you to use the `transfer` command.

## Usage

See [examples](./examples).

> [!NOTE]
> To use this crate in another crate, include this patch
> for [`merkle-tree-rs`](https://github.com/consensus-shipyard/merkle-tree-rs) in your `Cargo.toml`.
> ```toml
> [patch.crates-io]
> # Contains some API changes that the upstream has not merged.
> merkle-tree-rs = { git = "https://github.com/consensus-shipyard/merkle-tree-rs.git", branch = "dev" }
> ```

The next SDK release will fix this issue.

## Contributing

PRs accepted.

Small note: If editing the README, please conform to
the [standard-readme](https://github.com/RichardLitt/standard-readme) specification.

## License

MIT OR Apache-2.0, © 2024 ADM Contributors
