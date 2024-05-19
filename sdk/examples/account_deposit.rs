// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::env;

use anyhow::anyhow;
use fvm_shared::econ::TokenAmount;

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::{account::Account, network::Network};
use adm_signer::{key::parse_secret_key, AccountKind, Signer, Wallet};

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
    let provider = JsonRpcProvider::new_http(network.rpc_url()?, None)?;

    // Setup local wallet using private key from arg
    let mut signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?)?;
    signer.init_sequence(&provider).await?;

    // Deposit some calibration funds into the subnet
    let tx = Account::deposit(
        &signer,
        signer.address(),
        network.parent_subnet_config(None, None)?,
        TokenAmount::from_whole(1),
    )
    .await?;
    println!(
        "Deposited 1 tFIL; Transaction hash: {}",
        tx.transaction_hash
    );

    Ok(())
}
