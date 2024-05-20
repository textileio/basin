# The Amazing Data Machine (ADM)

[![License](https://img.shields.io/github/license/amazingdatamachine/adm.svg)](./LICENSE)
[![standard-readme compliant](https://img.shields.io/badge/standard--readme-OK-green.svg)](https://github.com/RichardLitt/standard-readme)

> ADM network interfaces & tooling for scalable subnets & onchain data storage

## Table of Contents

- [Background](#background)
  - [Architecture](#architecture)
    - [IPC \& subnets](#ipc--subnets)
    - [CometBFT \& state replication](#cometbft--state-replication)
    - [ABCI \& Fendermint](#abci--fendermint)
    - [Consensus](#consensus)
  - [Actors (smart contracts)](#actors-smart-contracts)
    - [Address manager](#address-manager)
    - [Object store](#object-store)
    - [Accumulator](#accumulator)
  - [Accounts](#accounts)
  - [Access control](#access-control)
  - [Broadcast modes](#broadcast-modes)
- [Usage](#usage)
  - [Chain RPCs \& funds](#chain-rpcs--funds)
  - [Limits](#limits)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

> [!CAUTION]
> Data may be **deleted every two weeks**! The ADM is currently an alpha testnet, and the network is subject to change biweekly with rolling updates. Please be aware that the network may be reset at any time. A more stable testnet will be released in the future that won't have this limitation.

## Background

The Amazing Data Machine (ADM) is a decentralized data layer, enabled by subnets that are purpose built for onchain data storage. Its built on top of the Filecoin Virtual Machine (FVM) and provides a horizontally scalable, verifiable, and cost effective data availability layer for onchain applications, networks (e.g., DePINs), and services. The first _data Layer 2 (L2)_. A handful of specialized "machines" (network-recognized actors) for object storage and data anchoring power the ADM's featured data services.

### Architecture

The ADM is developed using the Filecoin [InterPlanetary Consensus (IPC) framework](https://docs.ipc.space/). IPC is a new blockchain scaling solution and architectural design that is an alternative to existing L2 scaling solutions and based on the design principles of on-demand horizontal scaling.

#### IPC & subnets

IPC allows for permissionless creation of new blockchain subsystems called _subnets_. Subnets are organized hierarchically, enabling seamless internal communication and eliminating the need for cross-chain bridges. Each subnet can employ its specific consensus algorithm and inherit security from parent subnets, and this structure facilitates hosting or sharding applications based on performance or cost needs.

Operators of a subnet must run a full validator node for both the parent and the child subnet. The parent subnet is responsible for the security of the child subnet, and the child subnet is responsible for its own consensus and state transitions.

> [!NOTE]
> The ADM is currently a single child subnet rooted in its parent Filecoin Calibration testnet. Future versions of the ADM will align more closely with IPC's permissionless subnet spawning and configurable consensus mechanisms.

#### CometBFT & state replication

[CometBFT](https://docs.cometbft.com/v0.38/) (formerly known as _Tendermint_) helps the ADM achieve state machine replication across all nodes in the network. Its consensus algorithm is based on a variant of Practical Byzantine Fault Tolerance (PBFT) and relies on a round-robin proposer selection mechanism, while incorporating elements that improve on PBFT's performance and communication overhead. It is a fast, battled-tested, and well-designed consensus engine.

You can consider CometBFT as a standalone process that issues commands to [Fendermint](https://github.com/consensus-shipyard/ipc/tree/main/fendermint) and exposes a public JSON RPC API, much like the Ethereum JSON RPC API.

#### ABCI & Fendermint

The ADM's blockchain functionality is exposed as a unified ABCI++ application controlled by CometBFT.

[Application Blockchain Interfaces (ABCI)](https://docs.cometbft.com/v0.38/spec/abci/) programs are an interface between CometBFT and the actual state machine being replicated. That is, ABCIs implement deterministic state machines to be securely replicated by the CometBFT consensus engine. The "++" in ABCI++ refers to additional functionality that CometBFT enables compared to the original ABCI, which helps improve the overall scalability and feature surface area.

Fendermint is a specialized ABCI++ interface to the Filecoin Virtual Machines (FVM) and Ethereum-compatible FEVM. It exposes FEVM/FVM-specific functionality within subnets, allowing the ADM subnets behave like Filecoin but with custom parameters to greatly improve throughput and features. Fendermint is also a standalone process that includes:

- **Interpreters:** Responsible for handling commands from CometBFT.
- **Snapshots:** CAR files that can be offered to peers for quick chain sync.
- **IPLD resolver:** A libp2p-based service that is used to resolve CIDs from IPFS as well as the network of validator peers. [IPLD](https://ipld.io/) powers the network's ability to store and retrieve data in a content-addressable way.

#### Consensus

Checkpointed states of a subnet are pushed to the parent ("bottomup"), which is essential for the parent to validate the state transitions of the child. The ADM passes checkpointed headers to its parent and uses the CometBFT ledger to gather relevant signatures and data. Additionally, the ADM can contact the IPLD resolver & store to read and write data to its internal state so that it is IPLD addressable. There is also a "topdown" sync action; subnets must have a view of their parent's finality, which includes the latest block hash, power table information, and (in the future) cross-subnet message passing.

In general, data are represented as CIDs onchain (within an ADM machine's state), and the actual data is stored offchain in a node's local (networked) block store. The ADM uses the concept of a _detached payload_ asynchronous sync mechanism, which is a transaction that includes a CID reference to an object, but does not include the object data itself. When a detached payload is added to the chain, validators are required to download the object data from the network and verify that it matches the CID reference. This ensures that all validators have a copy of the object data and can verify the integrity of the object store state.

The core IPC process for topdown parent finality is a vote-based mechanism. Once a validator has the data locally (via synchronization with its peers), the validator issues a _vote_ to the other validators to confirm data availability (this is similar to Ethereum's concept of a [data availability committee](https://ethereum.org/en/developers/docs/data-availability/#data-availability-committees)). During this time, normal block production continues. The leader of a given consensus round checks the vote tally for quorum based on the power table (stake) within the subnet. If a quorum is reached, the leader injects a synthetic transaction into the proposal, which is validated by the other validators based on their view of the vote tally, and then finally during execution of this transaction, the object is marked as `resolved`.

### Actors (smart contracts)

The ADM's core functionality is enabled with with series of core actors, which are synonymous to smart contracts that run on the FVM and are used to manage, query, and update the state of the subnet. The FVM is responsible for executing the logic of the ADM protocol, including processing transactions, updating account balances, and managing the state of the network. There are three primary actors in the ADM:

- Address manager
- Object store
- Accumulator

The FVM executes messages in WASM over actor state and uses the [Wasmtime](https://github.com/bytecodealliance/wasmtime) runtime, and this includes a WASM implementation of the EVM bytecode interpreter. Under the hood, any "built-in" (e.g., the EVM addressing `t410`/`f410` actor described above) and "custom" (unique / subnet-specific) actors are compiled to CAR files and provided to the subnet during genesis.

#### Address manager

Users are able to deploy new machines on-demand using the address manager actor. This actor is responsible for creating new object stores or accumulators and also managing the state of the network. Each machine is associated with a unique address, which is used to identify the store on the network.

#### Object store

The object store actor provides a set of methods for interacting with the store, including `put`, `get`, and `delete`, which allow users to store and retrieve data from the store.

Internally, the state of an object store is represented as an [IPLD-based HAMT](ipns://ipld.io/specs/advanced-data-layouts/hamt/spec/) (hash array mapped trie). The IPLD data model provides a flexible and extensible way to represent complex data structures, and the invariants and mutation rules enforced by the IPLD HAMT provides the ability to maintain canonical forms given any set of keys and their values, regardless of insertion order and intermediate data insertion and deletion. Therefore, for any given set of keys and their values, you get a consistent object store configuration, such that the root node will always produce the same content identifier (CID).

#### Accumulator

The accumulator actors lets you use an append-only Merkle tree that summarizes the underlying data by a single `root`. As you push new data to the accumulator, you can retrieve the underlying data at a `leaf` or other relevant data structure components like `peaks` and total `count`. Similar to the object store actor, the accumulator stores a CID summary in its onchain state.

### Accounts

Accounts (ECDSA, secp256k1) are used to send data-carrying transactions as you would on any blockchain system. Since the ADM is built on top of Filecoin's FVM, the addresses follow a _slightly_ different convention than a purely EVM-based account system. Here's a quick primer:

- Addresses are prefixed with a network identifier: `t` for Filecoin testnet, or `f` for Filecoin mainnet, and there are five different address types denoted by the second character in the address string: `t0`/`f0` to `t4`/`f4`.
- If you're coming from the EVM world, you'll mostly see two types in the ADM:
  - `t2`/`f2`: Any FVM-native actor that gets deployed, such as the object store and accumlator machines.
  - `t4`/`f4`: A namespaced actor address, and `t410`/`f410` is a specifalized namespace for Ethereum-compatible addresses (wallets _and_ smart contracts) on the FVM.
- Namely, each `t410...`/`f410...` address is equivalent to a `0x` address; the hex address is [encoded in the FVM address string](https://docs.filecoin.io/smart-contracts/filecoin-evm-runtime/address-types#converting-to-a-0x-style-address).

Once your EVM-style account is registered on a subnet, the `0x` and its corresponding `t410...`/`f410...` addresses can be used interchangeably on Filecoin and all ADM subnets.

### Access control

Currently, there are two types of write access controls: only-owner, or public access. For example, you can create an object store that you and _only_ you can write to—gated by signatures from your private key. Or, you can have "allow all" access where anyone can write data.

This is being further refined, and there will more robust access control mechanisms in the future.

### Broadcast modes

For context, are three ways in which transactions can be sent/broadcasted to the network: `commit`, `sync`, and `async`. Here's a quick overview of each:

- `commit`: Wait until the transaction is delivered and final (default behavior).
- `sync`: Wait only for the result of a local transaction pre-check, but don’t wait for it to be delivered to all validators (i.e., added risk the transaction may fail during delivery).
- `async`: Does not wait at all. You will not see errors in your terminal (i.e., added risk the transaction may fail during delivery).

## Usage

The ADM comes with both a CLI and Rust SDK network interfaces. You can find detailed instructions for each of these below:

- CLI: [here](./cli/README.md)
- SDK: [here](./sdk/README.md)

### Chain RPCs & funds

Since the ADM is built on top of Filecoin, you must have FIL in your account to interact with the network. The ADM is currently only live on top of Filecoin Calibration, so you can get tFIL via the faucet [here](https://faucet.calibnet.chainsafe-fil.io/funds.html). For reference, Filecoin chain information can be found [here](https://chainlist.org/?search=filecoin&testnets=true).

### Limits

There are a couple of limitations and behaviors to be aware of:

- The maximum size for a single object is 1 GiB.
- Objects that are less than or equal to 1024 bytes are "internal" object that get stored fully onchain.
- Objects that are greater than 1024 bytes are "external" objects that get stored offchain and resolved at a CID.
- The current throughput of the ADM is hundreds of transactions per second (TPS) (note: this is a rough estimate and may vary based on network conditions, but the design is being further optimized).

## Development

When developing against a local network, be sure to set the `--network` (or `NETWORK`) to `devnet`. This presumes you have a local-only setup running, provided by the [`ipc`](https://github.com/amazingdatamachine/ipc) repo and custom actors in [`builtin-actors`](https://github.com/amazingdatamachine/builtin-actors).

All of the available commands include:

- Build all crates: `make build`
- Install the CLI: `make install`
- Run tests: `make test`
- Run linter: `make lint`
- Run formatter: `make check-fmt`
- Run clippy: `make check-clippy`
- Do all of the above: `make all`
- Clean dependencies: `make clean`

## Contributing

PRs accepted.

Small note: If editing the README, please conform to the [standard-readme](https://github.com/RichardLitt/standard-readme) specification.

## License

MIT OR Apache-2.0, © 2024 ADM Contributors
