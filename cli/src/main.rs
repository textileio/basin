// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

// TODO: Handle gas options
// TODO: Handle broadcast mode options

use std::str::FromStr;

use anyhow::anyhow;
use clap::{Args, Parser, Subcommand, ValueEnum};
use fvm_shared::address::Address;
use serde::Serialize;
use stderrlog::Timestamp;
use tendermint_rpc::Url;

use adm_provider::util::parse_address;
use adm_sdk::network::{
    use_testnet_addresses, DEVNET_RPC_URL, DEVNET_SUBNET_ID, TESTNET_PARENT_GATEWAY_ADDRESS,
    TESTNET_PARENT_REGISTRY_ADDRESS, TESTNET_PARENT_URL, TESTNET_RPC_URL, TESTNET_SUBNET_ID,
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
    /// Network presets for subnet and rpc_url.
    #[arg(long, value_enum, env, default_value_t = Network::Testnet)]
    network: Network,
    /// The ID of the target subnet. If not present, a value derived from the network flag is used.
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
enum Network {
    /// Network presets for mainnet.
    Mainnet,
    /// Network presets for Calibration (default pre-mainnet).
    Testnet,
    /// Network presets for local development.
    Devnet,
}

impl Network {
    fn subnet(&self) -> anyhow::Result<SubnetID> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(SubnetID::from_str(TESTNET_SUBNET_ID)?),
            Network::Devnet => Ok(SubnetID::from_str(DEVNET_SUBNET_ID)?),
        }
    }

    fn rpc_url(&self) -> anyhow::Result<Url> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(Url::from_str(TESTNET_RPC_URL)?),
            Network::Devnet => Ok(Url::from_str(DEVNET_RPC_URL)?),
        }
    }

    fn parent_url(&self) -> anyhow::Result<reqwest::Url> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(reqwest::Url::from_str(TESTNET_PARENT_URL)?),
            Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    fn parent_gateway(&self) -> anyhow::Result<Address> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(parse_address(TESTNET_PARENT_GATEWAY_ADDRESS)?),
            Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    fn parent_registry(&self) -> anyhow::Result<Address> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(parse_address(TESTNET_PARENT_REGISTRY_ADDRESS)?),
            Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }
}

#[derive(Clone, Debug, Args)]
#[group(required = true, multiple = false)]
pub struct NetworkArgs {
    /// Used for mainnet.
    #[arg(long, env)]
    mainnet: bool,
    /// Used for Calibration (default is 'true' pre-mainnet).
    #[arg(long, env, default_value_t = true)]
    testnet: bool,
    /// Used for local development.
    #[arg(long, env)]
    devnet: bool,
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
    if let Some(subnet) = cli.subnet.clone() {
        Ok(subnet)
    } else {
        cli.network.subnet()
    }
}

/// Returns rpc url from the override or network preset.
fn get_rpc_url(cli: &Cli) -> anyhow::Result<Url> {
    if let Some(url) = cli.rpc_url.clone() {
        Ok(url)
    } else {
        cli.network.rpc_url()
    }
}

/// Print serializable to stdout as pretty formatted JSON.
fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(&value)?;
    println!("{}", json);
    Ok(())
}
