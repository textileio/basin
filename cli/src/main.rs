// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use clap::{Args, Parser, Subcommand, ValueEnum};
use fvm_shared::econ::TokenAmount;
use serde::Serialize;
use stderrlog::Timestamp;
use tendermint_rpc::Url;

use adm_provider::{
    message::GasParams, util::parse_token_amount_from_atto, BroadcastMode as SDKBroadcastMode,
};
use adm_sdk::{
    network::{use_testnet_addresses, Network as SdkNetwork},
    TxParams,
};
use adm_signer::SubnetID;

use crate::account::{handle_account, AccountArgs};
use crate::machine::{handle_machine, MachineArgs};

mod account;
mod machine;

/// Command line args
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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Network {
    /// Network presets for mainnet.
    Mainnet,
    /// Network presets for Calibration (default pre-mainnet).
    Testnet,
    /// Network presets for local development.
    Devnet,
}

impl Network {
    pub fn get(&self) -> SdkNetwork {
        match self {
            Network::Mainnet => SdkNetwork::Mainnet,
            Network::Testnet => SdkNetwork::Testnet,
            Network::Devnet => SdkNetwork::Devnet,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BroadcastMode {
    /// Broadcast mode presets for Commit.
    Commit,
    /// Broadcast mode presets for Sync.
    Sync,
    /// Broadcast mode presets for Async.
    Async,
}

impl BroadcastMode {
    pub fn get(&self) -> SDKBroadcastMode {
        match self {
            BroadcastMode::Commit => SDKBroadcastMode::Commit,
            BroadcastMode::Sync => SDKBroadcastMode::Sync,
            BroadcastMode::Async => SDKBroadcastMode::Async,
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

    match cli.network {
        Network::Testnet | Network::Devnet => use_testnet_addresses(),
        _ => {}
    }

    match &cli.command.clone() {
        Commands::Account(args) => handle_account(cli, args).await,
        Commands::Machine(args) => handle_machine(cli, args).await,
    }
}

/// Returns subnet ID from the override or network preset.
fn get_subnet_id(cli: &Cli) -> anyhow::Result<SubnetID> {
    Ok(cli.subnet.clone().unwrap_or(cli.network.get().subnet()?))
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

/// Parse range CLI argument and return start and end byte positions.
fn parse_range_arg(range: String, size: u64) -> anyhow::Result<(u64, u64)> {
    let range: Vec<String> = range.split('-').map(|n| n.to_string()).collect();
    if range.len() != 2 {
        return Err(anyhow!("invalid range format"));
    }
    let (start, end): (u64, u64) = match (!range[0].is_empty(), !range[1].is_empty()) {
        (true, true) => (range[0].parse::<u64>()?, range[1].parse::<u64>()?),
        (true, false) => (range[0].parse::<u64>()?, size - 1),
        (false, true) => {
            let last = range[1].parse::<u64>()?;
            if last > size {
                (0, size - 1)
            } else {
                (size - last, size - 1)
            }
        }
        (false, false) => (0, size - 1),
    };
    if start > end || end >= size {
        return Err(anyhow!("invalid range"));
    }
    Ok((start, end))
}
