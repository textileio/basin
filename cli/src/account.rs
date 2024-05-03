// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use clap::{Args, Subcommand};
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_shared::{address::Address, econ::TokenAmount};

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::{account::Account, Adm, Recipient};
use adm_signer::Signer;

use crate::{get_signer, parse_address, parse_token_amount, print_json, Cli};

#[derive(Clone, Debug, Args)]
pub struct AccountArgs {
    #[command(subcommand)]
    command: AccountCommands,
}

#[derive(Clone, Debug, Subcommand)]
enum AccountCommands {
    /// List machines by owner.
    Machines(MachinesArgs),
    /// Transfer funds to another account.
    Transfer(TransferArgs),
}

#[derive(Clone, Debug, Args)]
struct MachinesArgs {
    /// Owner address. The signer address is used if no address is given.
    #[arg(short, long, value_parser = parse_address)]
    owner: Option<Address>,
}

#[derive(Clone, Debug, Args)]
pub struct TransferArgs {
    /// The recipient account address.
    #[arg(long, value_parser = parse_address)]
    pub to: Address,
    /// The amount to transfer in FIL.
    #[arg(value_parser = parse_token_amount)]
    pub amount: TokenAmount,
}

pub async fn handle_account(cli: Cli, args: &AccountArgs) -> anyhow::Result<()> {
    let provider = JsonRpcProvider::new_http(cli.rpc_url, None)?;
    let mut signer = get_signer(&provider, cli.wallet_pk, cli.chain_name).await?;

    match &args.command {
        AccountCommands::Machines(args) => {
            let owner = match args.owner {
                Some(addr) => addr,
                None => signer.address(),
            };
            let metadata = Account::machines(&provider, owner, FvmQueryHeight::Committed).await?;

            print_json(&metadata)
        }
        AccountCommands::Transfer(args) => {
            let to = Recipient::Address(args.to);
            let amount = args.amount.clone();

            let tx = Adm::transfer(&provider, &mut signer, to, amount, Default::default()).await?;

            print_json(&tx)
        }
    }
}
