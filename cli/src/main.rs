// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

// TODO: Handle gas options
// TODO: Handle broadcast mode options
// TODO: Add command for Adm::transfer
// TODO: Parse returned account addresses as EthAddress (hex)

use std::str::FromStr;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use fendermint_vm_core::chainid;
use fvm_shared::address::{Address, Error, Network};
use fvm_shared::econ::TokenAmount;
use ipc_api::ethers_address_to_fil_address;
use serde::Serialize;
use stderrlog::Timestamp;
use tendermint_rpc::Url;

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::network::use_testnet_addresses;
use adm_signer::{key::read_secret_key, AccountKind, Wallet};

use crate::machine::{handle_machine, MachineArgs};
use crate::subnet::{handle_subnet, SubnetArgs};

mod machine;
mod subnet;

// const MAX_INTERNAL_OBJECT_SIZE: usize = 1024;
const MAX_ACC_PAYLOAD_SIZE: usize = 1024 * 500;

/// Command line args
#[derive(Clone, Debug, Parser)]
#[command(name = "adm", author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Node CometBFT RPC URL.
    #[arg(long, env, default_value = "http://127.0.0.1:26657")]
    rpc_url: Url,
    /// Node Object API URL.
    #[arg(long, env, default_value = "http://127.0.0.1:8001")]
    object_api_url: Url,
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(long, env)]
    wallet_pk: Option<String>,
    /// IPC subnet chain name.
    #[arg(long, env, default_value = "test")]
    chain_name: String,
    /// Use testnet addresses (default is 'true' pre-mainnet).
    #[arg(long, env, default_value_t = true)]
    testnet: bool,
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
    /// Machine related commands.
    #[clap(alias = "machines")]
    Machine(MachineArgs),
    /// Subnet related commands.
    #[clap(alias = "subnets")]
    Subnet(SubnetArgs),
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

    if cli.testnet {
        use_testnet_addresses()
    }

    match &cli.command.clone() {
        Commands::Machine(args) => handle_machine(cli, args).await,
        Commands::Subnet(args) => handle_subnet(cli, args).await,
    }
}

/// Returns a wallet instance from a private key and chain name.
///
/// This method will fetch the current nonce for the associated address account.
async fn get_signer(
    provider: &JsonRpcProvider,
    pk: Option<String>,
    chain_name: String,
) -> anyhow::Result<Wallet> {
    if let Some(pk) = pk {
        let chain_id = chainid::from_str_hashed(&chain_name)?;
        let sk = read_secret_key(&pk)?;
        let mut wallet = Wallet::new_secp256k1(sk, AccountKind::Ethereum, chain_id)?;
        wallet.init_sequence(provider).await?;
        Ok(wallet)
    } else {
        Err(anyhow!(
            "--wallet-pk <WALLET_PK> is required to sign transactions"
        ))
    }
}

/// Clap parser for f/eth-address.
fn parse_address(s: &str) -> Result<Address, String> {
    let addr = Network::Mainnet
        .parse_address(s)
        .or_else(|e| match e {
            Error::UnknownNetwork => Network::Testnet.parse_address(s),
            _ => Err(e),
        })
        .or_else(|_| {
            let addr = ethers::types::Address::from_str(s)?;
            ethers_address_to_fil_address(&addr)
        })
        .map_err(|e| format!("{}", e))?;
    Ok(addr)
}

/// We only support up to 9 decimal digits for transaction.
const FIL_AMOUNT_NANO_DIGITS: u32 = 9;

/// Clap parser for token amount.
fn parse_token_amount(s: &str) -> Result<TokenAmount, String> {
    let f: f64 = s.parse().map_err(|e| format!("{}", e))?;
    // no rounding, just the integer part
    let nano = f64::trunc(f * (10u64.pow(FIL_AMOUNT_NANO_DIGITS) as f64));
    Ok(TokenAmount::from_nano(nano as u128))
}

/// Print serializable to stdout as pretty formatted JSON.
fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(&value)?;
    println!("{}", json);
    Ok(())
}
