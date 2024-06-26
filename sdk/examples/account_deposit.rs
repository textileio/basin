// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::env;

use anyhow::anyhow;
use ethers::utils::hex::ToHexExt;
use fvm_shared::econ::TokenAmount;

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
    // Note: The debit account _must_ hold at least 1 Calibration tFIL for the deposit
    // plus enough to cover the transaction fee.
    // Go to the faucet at https://faucet.calibnet.chainsafe-fil.io/ to get yourself some tFIL.
    let network = Network::Testnet.init();

    // Setup local wallet using private key from arg
    let signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?.parent()?)?;

    // Deposit some calibration funds into the subnet
    // Note: The debit account _must_ have Calibration
    let tx = Account::deposit(
        &signer,
        signer.address(),
        network.parent_subnet_config(Default::default())?,
        TokenAmount::from_whole(1),
    )
    .await?;

    println!(
        "Deposited 1 tFIL to {}",
        signer.evm_address()?.encode_hex_with_prefix()
    );
    println!(
        "Transaction hash: 0x{}",
        hex::encode(tx.transaction_hash.to_fixed_bytes())
    );

    Ok(())
}
