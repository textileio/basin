// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::env;

use anyhow::anyhow;
use bytes::Bytes;
use fendermint_actor_machine::WriteAccess;
use fendermint_vm_message::query::FvmQueryHeight;

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::{
    machine::{accumulator::Accumulator, Machine},
    network::Network,
};
use adm_signer::{key::parse_secret_key, AccountKind, Wallet};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(anyhow!("missing hex-encoded private key"));
    }
    let pk_kex = &args[1];
    let pk = parse_secret_key(pk_kex)?;

    // Use testnet network defaults
    let network = Network::Testnet;

    // Setup network provider
    let provider = JsonRpcProvider::new_http(network.rpc_url()?, None, None)?;

    // Setup local wallet using private key from arg
    let mut signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?)?;
    signer.init_sequence(&provider).await?;

    // Create a new accumulator
    let (machine, tx) = Accumulator::new(
        &provider,
        &mut signer,
        WriteAccess::OnlyOwner,
        Default::default(),
    )
    .await?;
    println!(
        "Created new accumulator {}; Transaction hash: {}",
        machine.address(),
        tx.hash
    );

    // Push a payload to the accumulator
    let payload = Bytes::from("my_payload");
    let tx = machine
        .push(&provider, &mut signer, payload, Default::default())
        .await?;
    println!(
        "Pushed payload to accumulator {}; Transaction hash: {}",
        machine.address(),
        tx.hash
    );

    // Query for the new root
    let root = machine.root(&provider, FvmQueryHeight::Committed).await?;
    println!("New accumulator root {}", root);

    Ok(())
}
