// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use clap::{error::ErrorKind, Args, CommandFactory, Subcommand};
use fendermint_crypto::SecretKey;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_shared::{address::Address, econ::TokenAmount};
use ipc_provider::config::subnet::EVMSubnet;
use reqwest::Url;
use serde_json::{json, Value};
use std::time::Duration;

use adm_provider::{
    json_rpc::JsonRpcProvider,
    util::{parse_address, parse_token_amount},
};
use adm_sdk::account::Account;
use adm_signer::{key::parse_secret_key, AccountKind, Signer, Void, Wallet};

use crate::{get_rpc_url, get_subnet_id, print_json, Cli};

#[derive(Clone, Debug, Args)]
pub struct AccountArgs {
    #[command(subcommand)]
    command: AccountCommands,
}

#[derive(Clone, Debug, Subcommand)]
enum AccountCommands {
    /// List machines by owner.
    Machines(MachinesArgs),
    /// Deposit funds into a subnet from its parent.
    Deposit(FundArgs),
    /// Withdraw funds from a subnet to its parent.
    Withdraw(FundArgs),
    /// Transfer funds to another account.
    Transfer(TransferArgs),
}

#[derive(Clone, Debug, Args)]
struct MachinesArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: Option<SecretKey>,
    /// Owner address. The signer address is used if no address is given.
    #[arg(long, value_parser = parse_address)]
    owner: Option<Address>,
}

#[derive(Clone, Debug, Args)]
pub struct FundArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: SecretKey,
    /// The recipient account address. If not present, the signer address is used.
    #[arg(long, value_parser = parse_address)]
    to: Option<Address>,
    /// The amount to transfer in FIL.
    #[arg(value_parser = parse_token_amount)]
    amount: TokenAmount,
    /// The parent rpc http endpoint.
    #[arg(long)]
    parent_url: Option<Url>,
    /// The parent gateway contract address.
    #[arg(long, value_parser = parse_address)]
    parent_gateway: Option<Address>,
    /// The parent registry contract address.
    #[arg(long, value_parser = parse_address)]
    parent_registry: Option<Address>,
    /// Timeout for calls to the parent Ethereum API.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "60s")]
    parent_timeout: Duration,
    /// Bearer token for any Authorization header.
    #[arg(long)]
    parent_auth_token: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct TransferArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: SecretKey,
    /// The recipient account address.
    #[arg(long, value_parser = parse_address)]
    to: Address,
    /// The amount to transfer in FIL.
    #[arg(value_parser = parse_token_amount)]
    amount: TokenAmount,
}

pub async fn handle_account(cli: Cli, args: &AccountArgs) -> anyhow::Result<()> {
    let provider = JsonRpcProvider::new_http(get_rpc_url(&cli)?, None)?;
    let subnet_id = get_subnet_id(&cli)?;

    match &args.command {
        AccountCommands::Machines(args) => {
            let metadata = if let Some(owner) = args.owner {
                Account::machines(&provider, &Void::new(owner), FvmQueryHeight::Committed).await?
            } else if let Some(sk) = args.private_key.clone() {
                let signer = Wallet::new_secp256k1(sk, AccountKind::Ethereum, subnet_id)?;
                Account::machines(&provider, &signer, FvmQueryHeight::Committed).await?
            } else {
                Cli::command()
                    .error(
                        ErrorKind::MissingRequiredArgument,
                        "the following required arguments were not provided: --private-key OR --owner",
                    )
                    .exit();
            };

            let metadata = metadata
                .iter()
                .map(|m| json!({"address": m.address.to_string(), "kind": m.kind}))
                .collect::<Vec<Value>>();

            print_json(&metadata)
        }
        AccountCommands::Transfer(args) => {
            let mut signer =
                Wallet::new_secp256k1(args.private_key.clone(), AccountKind::Ethereum, subnet_id)?;
            signer.init_sequence(&provider).await?;

            let tx = Account::transfer(
                &provider,
                &mut signer,
                args.to,
                args.amount.clone(),
                Default::default(),
            )
            .await?;

            print_json(&tx)
        }
        AccountCommands::Deposit(args) => {
            let config = get_subnet_config(&cli, args)?;

            let signer =
                Wallet::new_secp256k1(args.private_key.clone(), AccountKind::Ethereum, subnet_id)?;

            let tx = Account::deposit(
                &signer,
                args.to.unwrap_or(signer.address()),
                config,
                args.amount.clone(),
            )
            .await?;

            print_json(&tx)
        }
        AccountCommands::Withdraw(args) => {
            let config = get_subnet_config(&cli, args)?;

            let signer =
                Wallet::new_secp256k1(args.private_key.clone(), AccountKind::Ethereum, subnet_id)?;

            let tx = Account::withdraw(
                &signer,
                args.to.unwrap_or(signer.address()),
                config,
                args.amount.clone(),
            )
            .await?;

            print_json(&tx)
        }
    }
}

/// Returns a subnet configuration from args.
fn get_subnet_config(cli: &Cli, args: &FundArgs) -> anyhow::Result<EVMSubnet> {
    Ok(EVMSubnet {
        provider_http: get_parent_url(cli, args.parent_url.clone())?,
        provider_timeout: Some(args.parent_timeout),
        auth_token: args.parent_auth_token.clone(),
        registry_addr: get_parent_registry(cli, args.parent_registry)?,
        gateway_addr: get_parent_gateway(cli, args.parent_gateway)?,
    })
}

/// Returns parent url from the override or network preset.
fn get_parent_url(cli: &Cli, url: Option<Url>) -> anyhow::Result<Url> {
    if let Some(url) = url {
        Ok(url)
    } else {
        cli.network.parent_url()
    }
}

/// Returns parent gateway from the override or network preset.
fn get_parent_gateway(cli: &Cli, addr: Option<Address>) -> anyhow::Result<Address> {
    if let Some(addr) = addr {
        Ok(addr)
    } else {
        cli.network.parent_gateway()
    }
}

/// Returns parent registry from the override or network preset.
fn get_parent_registry(cli: &Cli, addr: Option<Address>) -> anyhow::Result<Address> {
    if let Some(addr) = addr {
        Ok(addr)
    } else {
        cli.network.parent_registry()
    }
}
