// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::env;

use anyhow::anyhow;
use fendermint_actor_machine::WriteAccess;
use fendermint_actor_objectstore::ObjectListItem;
use rand::{thread_rng, Rng};

use adm_provider::json_rpc::JsonRpcProvider;
use adm_sdk::machine::objectstore::QueryOptions;
use adm_sdk::{
    machine::{objectstore::ObjectStore, Machine},
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
    let provider = JsonRpcProvider::new_http(network.rpc_url()?, None)?;

    // Setup local wallet using private key from arg
    let mut signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?)?;
    signer.init_sequence(&provider).await?;

    // Create a new object store
    let (machine, tx) = ObjectStore::new(
        &provider,
        &mut signer,
        WriteAccess::OnlyOwner,
        Default::default(),
    )
    .await?;
    println!(
        "Created new object store {}; Transaction hash: {}",
        machine.address(),
        tx.hash
    );

    // Create a temp file to add
    let file = async_tempfile::TempFile::new().await?;
    let mut rng = thread_rng();
    let mut random_data = vec![0; 1024 * 1024]; // 1 MiB
    rng.fill(&mut random_data[..]);

    // Add a file to the object store
    let key = "foo/my_file";
    let tx = machine
        .add(
            &provider,
            &mut signer,
            network.object_api_url()?,
            key,
            file,
            Default::default(),
        )
        .await?;
    println!(
        "Added 1MiB file to object store {} with key {}; Transaction hash: {}",
        machine.address(),
        key,
        tx.hash
    );

    // Query for the object
    let mut options = QueryOptions::default();
    options.prefix = "foo".into();
    let query = machine.query(&provider, options).await?;
    if let Some(list) = query {
        for obj in list.objects {
            let key = core::str::from_utf8(&obj.0).unwrap_or_default();
            match &obj.1 {
                ObjectListItem::Internal((cid, size)) => {
                    println!("{}: {} (internal; size = {})", key, cid, size);
                }
                ObjectListItem::External((cid, resolved)) => {
                    // `resolved` indicates the validators were able to fetch and verify the file
                    println!("{}: {} (detached; resolved = {})", key, cid, resolved);
                }
            }
        }
    }

    Ok(())
}
