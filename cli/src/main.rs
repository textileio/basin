// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

// TODO: Handle gas options
// TODO: Handle broadcast mode options
// TODO: Add command for Adm::transfer
// TODO: Add doc strings for commands (they show with --help)
// TODO: Parse returned account addresses as EthAddress (hex)

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use fendermint_vm_core::chainid;
use fvm_shared::address::{Address, Network};
use serde::Serialize;
use stderrlog::Timestamp;
use tendermint_rpc::Url;

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::network::use_testnet_addresses;
use adm_signer::{key::read_secret_key, AccountKind, Wallet};

use crate::machine::{handle_machine, MachineArgs};

mod machine;

// use tokio_util::codec::{BytesCodec, FramedRead};
// use reqwest::Body;

// const MAX_INTERNAL_OBJECT_SIZE: usize = 1024;
const MAX_ACC_PAYLOAD_SIZE: usize = 1024 * 500;

/// Command line args
#[derive(Clone, Debug, Parser)]
#[command(name = "adm", author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    /// Node CometBFT RPC URL
    #[arg(long, env, default_value = "http://127.0.0.1:26657")]
    rpc_url: Url,
    /// Node Object API URL
    #[arg(long, env, default_value = "http://127.0.0.1:8001")]
    object_api_url: Url,
    /// Wallet private key (ECDSA, secp256k1) for signing transactions
    #[arg(long, env)]
    wallet_pk: Option<String>,
    /// IPC chain name
    #[arg(long, env, default_value = "test")]
    chain_name: String,
    /// Use testnet addresses
    #[arg(long, env, default_value_t = true)]
    testnet: bool,
    /// Logging verbosity (repeat for more verbose logging)
    #[arg(short, long, env, action = clap::ArgAction::Count)]
    verbosity: u8,
    /// Silence logging
    #[arg(short, long, env, default_value_t = false)]
    quiet: bool,
}

#[derive(Clone, Debug, Subcommand)]
enum Commands {
    #[clap(alias = "machines")]
    Machine(MachineArgs),
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
    }
}

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

pub fn parse_address(s: &str) -> Result<Address, String> {
    let addr = Network::Mainnet
        .parse_address(s)
        .or_else(|_| Network::Testnet.parse_address(s))
        .map_err(|e| format!("{}", e))?;
    Ok(addr)
}

fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(&value)?;
    println!("{}", json);
    Ok(())
}
