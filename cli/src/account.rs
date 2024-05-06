// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use clap::{error::ErrorKind, Args, CommandFactory, Subcommand};
use fendermint_crypto::SecretKey;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_shared::{address::Address, econ::TokenAmount};
use reqwest::Url;
use serde_json::{json, Value};
use std::time::Duration;

use adm_provider::{
    json_rpc::JsonRpcProvider,
    util::{parse_address, parse_token_amount},
};
use adm_sdk::{account::Account, ipc::subnet::EVMSubnet};
use adm_signer::{key::parse_secret_key, AccountKind, Signer, SubnetID, Void, Wallet};

use crate::{get_rpc_url, get_subnet_id, print_json, Cli};

#[derive(Clone, Debug, Args)]
pub struct AccountArgs {
    #[command(subcommand)]
    command: AccountCommands,
}

#[derive(Clone, Debug, Subcommand)]
enum AccountCommands {
    /// List machines by owner in a subnet.
    Machines(AddressArgs),
    /// Get account sequence in a subnet.
    Sequence(AddressArgs),
    /// Get account balance in a subnet.
    Balance(AddressArgs),
    /// Get account balance on subnet's parent.
    ParentBalance(ParentBalanceArgs),
    /// Deposit funds into a subnet from its parent.
    Deposit(FundArgs),
    /// Withdraw funds from a subnet to its parent.
    Withdraw(FundArgs),
    /// Transfer funds to another account in a subnet.
    Transfer(TransferArgs),
}

#[derive(Clone, Debug, Args)]
struct AddressArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: Option<SecretKey>,
    /// Owner address. The signer address is used if no address is given.
    #[arg(short, long, value_parser = parse_address)]
    address: Option<Address>,
}

#[derive(Clone, Debug, Args)]
struct ParentBalanceArgs {
    #[command(flatten)]
    address: AddressArgs,
    #[command(flatten)]
    subnet: SubnetArgs,
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
    #[command(flatten)]
    subnet: SubnetArgs,
}

#[derive(Clone, Debug, Args)]
pub struct SubnetArgs {
    /// The Ethereum API rpc http endpoint.
    #[arg(long)]
    evm_rpc_url: Option<Url>,
    /// Timeout for calls to the Ethereum API.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "60s")]
    evm_rpc_timeout: Duration,
    /// Bearer token for any Authorization header.
    #[arg(long)]
    evm_rpc_auth_token: Option<String>,
    /// The gateway contract address.
    #[arg(long, value_parser = parse_address)]
    evm_gateway: Option<Address>,
    /// The registry contract address.
    #[arg(long, value_parser = parse_address)]
    evm_registry: Option<Address>,
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
            let address = get_address(args.clone(), &subnet_id)?;
            let metadata =
                Account::machines(&provider, &Void::new(address), FvmQueryHeight::Committed)
                    .await?;

            let metadata = metadata
                .iter()
                .map(|m| json!({"address": m.address.to_string(), "kind": m.kind}))
                .collect::<Vec<Value>>();

            print_json(&metadata)
        }
        AccountCommands::Sequence(args) => {
            let address = get_address(args.clone(), &subnet_id)?;
            let sequence =
                Account::sequence(&provider, &Void::new(address), FvmQueryHeight::Committed)
                    .await?;

            print_json(&json!({"sequence": sequence}))
        }
        AccountCommands::Balance(args) => {
            let address = get_address(args.clone(), &subnet_id)?;
            let balance =
                Account::balance(&provider, &Void::new(address), FvmQueryHeight::Committed).await?;

            print_json(&json!({"balance": balance.to_string()}))
        }
        AccountCommands::ParentBalance(args) => {
            let address = get_address(args.address.clone(), &subnet_id)?;
            let config = get_parent_subnet_config(&cli, &subnet_id, args.subnet.clone())?;
            let balance = Account::parent_balance(&Void::new(address), config).await?;

            print_json(&json!({"balance": balance.to_string()}))
        }
        AccountCommands::Deposit(args) => {
            let config = get_parent_subnet_config(&cli, &subnet_id, args.subnet.clone())?;

            let signer = Wallet::new_secp256k1(
                args.private_key.clone(),
                AccountKind::Ethereum,
                subnet_id.parent()?, // Signer must target the parent subnet
            )?;

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
            let config = get_subnet_config(&cli, &subnet_id, args.subnet.clone())?;

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

/// Returns the subnet configuration from args.
fn get_subnet_config(cli: &Cli, id: &SubnetID, args: SubnetArgs) -> anyhow::Result<EVMSubnet> {
    let network = cli.network.get();
    Ok(EVMSubnet {
        id: id.clone(),
        provider_http: args.evm_rpc_url.unwrap_or(network.evm_rpc_url()?),
        provider_timeout: Some(args.evm_rpc_timeout),
        auth_token: args.evm_rpc_auth_token,
        registry_addr: args.evm_registry.unwrap_or(network.evm_registry()?),
        gateway_addr: args.evm_gateway.unwrap_or(network.evm_gateway()?),
    })
}

/// Returns the parent subnet configuration from args.
fn get_parent_subnet_config(
    cli: &Cli,
    id: &SubnetID,
    args: SubnetArgs,
) -> anyhow::Result<EVMSubnet> {
    let network = cli.network.get();
    Ok(EVMSubnet {
        id: id.clone(),
        provider_http: args.evm_rpc_url.unwrap_or(network.parent_evm_rpc_url()?),
        provider_timeout: Some(args.evm_rpc_timeout),
        auth_token: args.evm_rpc_auth_token,
        registry_addr: args.evm_registry.unwrap_or(network.parent_evm_registry()?),
        gateway_addr: args.evm_gateway.unwrap_or(network.parent_evm_gateway()?),
    })
}
