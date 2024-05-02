// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::time::Duration;

use clap::{Args, Subcommand};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use ipc_api::subnet_id::SubnetID;
use ipc_provider::config::{
    subnet::{EVMSubnet, SubnetConfig},
    Subnet,
};
use reqwest::Url;

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::{Adm, TxRecipient};

use crate::{get_signer, parse_address, parse_token_amount, print_json, Cli};

#[derive(Clone, Debug, Args)]
pub struct SubnetArgs {
    #[command(subcommand)]
    command: SubnetCommands,
    /// The ID of the target subnet.
    #[arg(long)]
    pub subnet: SubnetID,
    /// The parent gateway address.
    #[arg(long, value_parser = parse_address)]
    pub parent_gateway: Address,
    /// The parent rpc http endpoint.
    #[arg(long, default_value = "https://api.calibration.node.glif.io/rpc/v1")]
    pub parent_http_endpoint: Url,
    /// Timeout for calls to the parent Ethereum API.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "60s")]
    pub parent_timeout: Duration,
    /// Bearer token for any Authorization header.
    #[arg(long)]
    pub parent_http_auth_token: Option<String>,
}

#[derive(Clone, Debug, Subcommand)]
enum SubnetCommands {
    /// Deposit funds into a subnet from its parent.
    Deposit(FundArgs),
    /// Withdraw funds from a subnet to its parent.
    Withdraw(FundArgs),
}

#[derive(Clone, Debug, Args)]
pub struct FundArgs {
    /// The deposit account address. The depositor address is used if no account is given.
    #[arg(long, value_parser = parse_address)]
    pub to: Option<Address>,
    /// The amount to deposit in FIL.
    #[arg(value_parser = parse_token_amount)]
    pub amount: TokenAmount,
}

pub async fn handle_subnet(cli: Cli, args: &SubnetArgs) -> anyhow::Result<()> {
    let provider = JsonRpcProvider::new_http(cli.rpc_url, None)?;
    let signer = get_signer(&provider, cli.wallet_pk, cli.chain_name).await?;
    let subnet = get_subnet(args);

    let fargs = match &args.command {
        SubnetCommands::Deposit(fargs) => fargs,
        SubnetCommands::Withdraw(fargs) => fargs,
    };
    let to = fargs
        .to
        .map(TxRecipient::Address)
        .unwrap_or(TxRecipient::Signer);
    let amount = fargs.amount.clone();

    let tx = match &args.command {
        SubnetCommands::Deposit(_) => Adm::deposit(&signer, to, subnet, amount).await?,
        SubnetCommands::Withdraw(_) => Adm::withdraw(&signer, to, subnet, amount).await?,
    };

    print_json(&serde_json::to_value(&tx)?)?;
    Ok(())
}

/// Returns a subnet configuration from args.
fn get_subnet(args: &SubnetArgs) -> Subnet {
    Subnet {
        id: args.subnet.clone(),
        config: SubnetConfig::Fevm(EVMSubnet {
            provider_http: args.parent_http_endpoint.clone(),
            provider_timeout: Some(args.parent_timeout),
            auth_token: args.parent_http_auth_token.clone(),
            registry_addr: Address::new_id(0), // Currently not used
            gateway_addr: args.parent_gateway,
        }),
    }
}
