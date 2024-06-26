// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use clap::{error::ErrorKind, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use fendermint_crypto::SecretKey;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_shared::{address::Address, econ::TokenAmount};
use serde::Serialize;
use stderrlog::Timestamp;
use tendermint_rpc::Url;

use adm_provider::{
    message::GasParams,
    tx::BroadcastMode as SDKBroadcastMode,
    util::{parse_address, parse_query_height, parse_token_amount_from_atto},
};
use adm_sdk::{network::Network as SdkNetwork, TxParams};
use adm_signer::{key::parse_secret_key, AccountKind, Signer, SubnetID, Wallet};

use crate::account::{handle_account, AccountArgs};
use crate::machine::{
    accumulator::{handle_accumulator, AccumulatorArgs},
    handle_machine,
    objectstore::{handle_objectstore, ObjectstoreArgs},
    MachineArgs,
};

mod account;
mod machine;

#[derive(Clone, Debug, Parser)]
#[command(name = "adm", author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Network presets for subnet and RPC URLs.
    #[arg(short, long, env, value_enum, default_value_t = Network::Testnet)]
    network: Network,
    /// The ID of the target subnet.
    #[arg(short, long, env)]
    subnet: Option<SubnetID>,
    /// Node CometBFT RPC URL.
    #[arg(long, env)]
    rpc_url: Option<Url>,
    /// Logging verbosity (repeat for more verbose logging).
    #[arg(short, long, env, action = clap::ArgAction::Count)]
    verbosity: u8,
    /// Silence logging.
    #[arg(short, long, env, default_value_t = false)]
    quiet: bool,
}

#[derive(Clone, Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Account related commands.
    #[clap(alias = "accounts")]
    Account(AccountArgs),
    /// Machine related commands.
    #[clap(alias = "machines")]
    Machine(MachineArgs),
    /// Object store related commands (alias: os).
    #[clap(alias = "os")]
    Objectstore(ObjectstoreArgs),
    /// Accumulator related commands (alias: ac).
    #[clap(alias = "ac")]
    Accumulator(AccumulatorArgs),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Network {
    /// Network presets for mainnet.
    Mainnet,
    /// Network presets for Calibration (default pre-mainnet).
    Testnet,
    /// Network presets for a local three-node network.
    Localnet,
    /// Network presets for local development.
    Devnet,
}

impl Network {
    pub fn get(&self) -> SdkNetwork {
        match self {
            Network::Mainnet => SdkNetwork::Mainnet,
            Network::Testnet => SdkNetwork::Testnet,
            Network::Localnet => SdkNetwork::Localnet,
            Network::Devnet => SdkNetwork::Devnet,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum BroadcastMode {
    /// Return immediately after the transaction is broadcasted without waiting for check results.
    Async,
    /// Wait for the check results before returning from broadcast.
    Sync,
    /// Wait for the delivery results before returning from broadcast.
    Commit,
}

impl BroadcastMode {
    pub fn get(&self) -> SDKBroadcastMode {
        match self {
            BroadcastMode::Async => SDKBroadcastMode::Async,
            BroadcastMode::Sync => SDKBroadcastMode::Sync,
            BroadcastMode::Commit => SDKBroadcastMode::Commit,
        }
    }
}

#[derive(Clone, Debug, Args)]
struct TxArgs {
    /// Gas limit for the transaction.
    #[arg(long, env)]
    gas_limit: Option<u64>,
    /// Maximum gas fee for the transaction in attoFIL.
    /// 1FIL = 10**18 attoFIL.
    #[arg(long, env, value_parser = parse_token_amount_from_atto)]
    gas_fee_cap: Option<TokenAmount>,
    /// Gas premium for the transaction in attoFIL.
    /// 1FIL = 10**18 attoFIL.
    #[arg(long, env, value_parser = parse_token_amount_from_atto)]
    gas_premium: Option<TokenAmount>,
    /// Sequence for the transaction.
    #[arg(long)]
    sequence: Option<u64>,
}

impl TxArgs {
    /// Creates transaction params from tx related CLI arguments.
    pub fn to_tx_params(&self) -> TxParams {
        TxParams {
            sequence: self.sequence,
            gas_params: GasParams {
                gas_limit: self.gas_limit.unwrap_or(fvm_shared::BLOCK_GAS_LIMIT),
                gas_fee_cap: self.gas_fee_cap.clone().unwrap_or_default(),
                gas_premium: self.gas_premium.clone().unwrap_or_default(),
            },
        }
    }
}

#[derive(Clone, Debug, Args)]
struct AddressArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: Option<SecretKey>,
    /// Account address. The signer address is used if no address is given.
    #[arg(short, long, value_parser = parse_address)]
    address: Option<Address>,
    /// Query block height.
    /// Possible values:
    /// "committed" (latest committed block),
    /// "pending" (consider pending state changes),
    /// or a specific block height, e.g., "123".
    #[arg(long, value_parser = parse_query_height, default_value = "committed")]
    height: FvmQueryHeight,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .quiet(cli.quiet)
        .verbosity(cli.verbosity as usize)
        .timestamp(Timestamp::Millisecond)
        .init()
        .unwrap();

    cli.network.get().init();

    match &cli.command.clone() {
        Commands::Account(args) => handle_account(cli, args).await,
        Commands::Objectstore(args) => handle_objectstore(cli, args).await,
        Commands::Accumulator(args) => handle_accumulator(cli, args).await,
        Commands::Machine(args) => handle_machine(cli, args).await,
    }
}

/// Returns address from private key or address arg.
fn get_address(args: AddressArgs, subnet_id: &SubnetID) -> anyhow::Result<Address> {
    let address = if let Some(addr) = args.address {
        addr
    } else if let Some(sk) = args.private_key.clone() {
        let signer = Wallet::new_secp256k1(sk, AccountKind::Ethereum, subnet_id.clone())?;
        signer.address()
    } else {
        Cli::command()
            .error(
                ErrorKind::MissingRequiredArgument,
                "the following required arguments were not provided: --private-key OR --address",
            )
            .exit();
    };
    Ok(address)
}

/// Returns subnet ID from the override or network preset.
fn get_subnet_id(cli: &Cli) -> anyhow::Result<SubnetID> {
    Ok(cli.subnet.clone().unwrap_or(cli.network.get().subnet_id()?))
}

/// Returns rpc url from the override or network preset.
fn get_rpc_url(cli: &Cli) -> anyhow::Result<Url> {
    Ok(cli.rpc_url.clone().unwrap_or(cli.network.get().rpc_url()?))
}

/// Print serializable to stdout as pretty formatted JSON.
fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(&value)?;
    println!("{}", json);
    Ok(())
}
