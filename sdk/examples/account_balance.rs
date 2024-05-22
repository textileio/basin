// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::env;

use anyhow::anyhow;
use ethers::utils::hex::ToHexExt;

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
    let network = Network::Testnet.init();

    // Setup local wallet using private key from arg
    let signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?.parent()?)?;

    // Deposit some calibration funds into the subnet
    // Note: The debit account _must_ have Calibration
    let balance = Account::balance(&signer, network.subnet_config(Default::default())?).await?;

    println!(
        "Balance of {}: {}",
        signer.evm_address()?.encode_hex_with_prefix(),
        balance,
    );

    Ok(())
}
