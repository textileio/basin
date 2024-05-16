// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use clap::{Args, Subcommand};
use ethers::utils::hex::ToHexExt;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_shared::address::Address;
use serde_json::json;

use adm_provider::{
    json_rpc::JsonRpcProvider,
    util::{get_delegated_address, parse_address, parse_query_height},
};
use adm_sdk::machine::DefaultMachine;

use crate::machine::{
    accumulator::{handle_accumulator, AccumulatorArgs},
    objectstore::{handle_objectstore, ObjectstoreArgs},
};
use crate::{get_rpc_url, print_json, Cli};

pub mod accumulator;
pub mod objectstore;

#[derive(Clone, Debug, Args)]
pub struct MachineArgs {
    #[command(subcommand)]
    command: MachineCommands,
}

#[derive(Clone, Debug, Subcommand)]
enum MachineCommands {
    /// Get machine info.
    Get(GetMachineArgs),
    /// Object store related commands.
    Objectstore(ObjectstoreArgs),
    /// Accumulator related commands.
    Accumulator(AccumulatorArgs),
}

#[derive(Clone, Debug, Args)]
struct GetMachineArgs {
    /// Machine address.
    #[arg(value_parser = parse_address)]
    address: Address,
    /// Query block height.
    #[arg(short, long, value_parser = parse_query_height, default_value = "committed")]
    height: FvmQueryHeight,
}

pub async fn handle_machine(cli: Cli, args: &MachineArgs) -> anyhow::Result<()> {
    match &args.command {
        MachineCommands::Get(args) => {
            let provider = JsonRpcProvider::new_http(get_rpc_url(&cli)?, None)?;
            let metadata = DefaultMachine::metadata(&provider, args.address, args.height).await?;

            let owner = get_delegated_address(metadata.owner)?.encode_hex_with_prefix();
            print_json(&json!({"kind": metadata.kind, "owner": owner}))
        }
        MachineCommands::Objectstore(args) => handle_objectstore(cli, args).await,
        MachineCommands::Accumulator(args) => handle_accumulator(cli, args).await,
    }
}
