// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use cid::Cid;
use clap::{Args, Parser, Subcommand};
use clap_stdin::FileOrStdin;
use console::Emoji;
use fendermint_actor_machine::WriteAccess;
use fendermint_actor_objectstore::{
    DeleteParams, GetParams, ListParams, Object, ObjectKind, ObjectListItem, PutParams,
};
use fendermint_crypto::SecretKey;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::serde_bytes::ByteBuf;
use fvm_shared::address::Address;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Value};
use tendermint_rpc::Url;
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

use adm_provider::{
    json_rpc::JsonRpcProvider,
    object::ObjectClient,
    util::{parse_address, parse_query_height},
};
use adm_sdk::{
    machine::{
        objectstore::{self, ObjectStore},
        Machine,
    },
    TxParams,
};
use adm_signer::{key::parse_secret_key, AccountKind, Wallet};

use crate::{get_rpc_url, get_subnet_id, parse_range_arg, print_json, BroadcastMode, Cli, TxArgs};

const MAX_INTERNAL_OBJECT_LENGTH: usize = 1024;

#[derive(Clone, Debug, Args)]
pub struct ObjectstoreArgs {
    #[command(subcommand)]
    command: ObjectstoreCommands,
}

#[derive(Clone, Debug, Subcommand)]
enum ObjectstoreCommands {
    /// Create a new object store.
    Create(ObjectstoreCreateArgs),
    /// Put an object with a key prefix.
    Put(ObjectstorePutArgs),
    /// Delete an object.
    Delete(ObjectstoreDeleteArgs),
    /// Get an object.
    Get(ObjectstoreGetArgs),
    /// List objects.
    List(ObjectstoreListArgs),
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreCreateArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: SecretKey,
    /// Allow public write access to the object store.
    #[arg(long, default_value_t = false)]
    public_write: bool,
    #[command(flatten)]
    tx_args: TxArgs,
}

#[derive(Clone, Debug, Parser)]
struct ObjectstorePutArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: SecretKey,
    /// Node Object API URL.
    #[arg(long, env)]
    object_api_url: Option<Url>,
    /// Object store machine address.
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Key of the object to upload.
    #[arg(short, long)]
    key: String,
    /// Overwrite the object if it already exists.
    #[arg(short, long)]
    overwrite: bool,
    /// Input file (or stdin) containing the object to upload.
    #[clap(default_value = "-")]
    input: FileOrStdin,
    /// Broadcast mode for the transaction.
    #[arg(short, long, value_enum, env, default_value_t = BroadcastMode::Commit)]
    broadcast_mode: BroadcastMode,
    #[command(flatten)]
    tx_args: TxArgs,
}

#[derive(Clone, Debug, Parser)]
struct ObjectstoreDeleteArgs {
    /// Wallet private key (ECDSA, secp256k1) for signing transactions.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    private_key: SecretKey,
    /// Object store machine address.
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Key of the object to delete.
    key: String,
    /// Broadcast mode for the transaction.
    #[arg(short, long, value_enum, env, default_value_t = BroadcastMode::Commit)]
    broadcast_mode: BroadcastMode,
    #[command(flatten)]
    tx_args: TxArgs,
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreAddressArgs {
    /// Object store machine address.
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Query block height.
    /// Possible values:
    /// "committed" (latest committed block),
    /// "pending" (consider pending state changes),
    /// or a specific block height, e.g., "123".
    #[arg(long, value_parser = parse_query_height, default_value = "committed")]
    height: FvmQueryHeight,
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreGetArgs {
    /// Node Object API URL.
    #[arg(long, env)]
    object_api_url: Option<Url>,
    /// Object store machine address.
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Key of the object to get.
    key: String,
    /// Range of bytes to get from the object.
    /// Format: "start-end" (inclusive).
    /// Example: "0-99" (first 100 bytes).
    #[arg(short, long)]
    range: Option<String>,
    /// Query block height.
    /// Possible values:
    /// "committed" (latest committed block),
    /// "pending" (consider pending state changes),
    /// or a specific block height, e.g., "123".
    #[arg(long, value_parser = parse_query_height, default_value = "committed")]
    height: FvmQueryHeight,
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreListArgs {
    /// Object store machine address.
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// The prefix to filter objects by.
    #[arg(short, long, default_value = "")]
    prefix: String,
    /// The delimiter used to define object hierarchy.
    #[arg(short, long, default_value = "/")]
    delimiter: String,
    /// The offset from which to start listing objects.
    #[arg(short, long, default_value_t = 0)]
    offset: u64,
    /// The maximum number of objects to list. '0' indicates max (10k).
    #[arg(short, long, default_value_t = 0)]
    limit: u64,
    /// Query block height.
    /// Possible values:
    /// "committed" (latest committed block),
    /// "pending" (consider pending state changes),
    /// or a specific block height, e.g., "123".
    #[arg(long, value_parser = parse_query_height, default_value = "committed")]
    height: FvmQueryHeight,
}

/// Objectstore commmands handler.
pub async fn handle_objectstore(cli: Cli, args: &ObjectstoreArgs) -> anyhow::Result<()> {
    let provider = JsonRpcProvider::new_http(get_rpc_url(&cli)?, None)?;
    let subnet_id = get_subnet_id(&cli)?;

    match &args.command {
        ObjectstoreCommands::Create(ObjectstoreCreateArgs {
            private_key,
            public_write,
            tx_args,
        }) => {
            let TxParams {
                sequence,
                gas_params,
            } = tx_args.to_tx_params();
            let mut signer =
                Wallet::new_secp256k1(private_key.clone(), AccountKind::Ethereum, subnet_id)?;
            signer.set_sequence(sequence, &provider).await?;

            let write_access = if *public_write {
                WriteAccess::Public
            } else {
                WriteAccess::OnlyOwner
            };
            let (store, tx) =
                ObjectStore::new(&provider, &mut signer, write_access, gas_params).await?;

            print_json(&json!({"address": store.address().to_string(), "tx": &tx}))
        }
        ObjectstoreCommands::Put(ObjectstorePutArgs {
            private_key,
            object_api_url,
            key,
            address,
            overwrite,
            input,
            broadcast_mode,
            tx_args,
        }) => {
            let TxParams {
                sequence,
                gas_params,
            } = tx_args.to_tx_params();
            let broadcast_mode = broadcast_mode.get();
            let mut signer = Wallet::new_secp256k1(
                private_key.clone(),
                AccountKind::Ethereum,
                subnet_id.clone(),
            )?;
            signer.set_sequence(sequence, &provider).await?;

            let machine = ObjectStore::attach(*address);
            let object_api_url = object_api_url
                .clone()
                .unwrap_or(cli.network.get().object_api_url()?);
            let object_client = ObjectClient::new(object_api_url);

            let mut reader = input.into_async_reader().await?;
            let mut first_chunk = vec![0; MAX_INTERNAL_OBJECT_LENGTH + 1];

            let upload_progress = ObjectProgressBar::new(cli.quiet);

            match reader.read(&mut first_chunk).await {
                Ok(first_chunk_size) => {
                    if first_chunk_size > MAX_INTERNAL_OBJECT_LENGTH {
                        let (tx, rx) = mpsc::channel::<Vec<u8>>(1024);
                        // Preprocess Object before uploading
                        upload_progress.show_processing();
                        let (object_cid, bytes_read) =
                            objectstore::process_object(reader, tx, first_chunk).await?;

                        // Upload Object with signature
                        upload_progress.show_uploading();
                        let response_cid = machine
                            .upload(
                                &mut signer,
                                object_client,
                                key.clone(),
                                object_cid,
                                first_chunk_size + bytes_read,
                                *overwrite,
                                subnet_id.chain_id(),
                                rx,
                            )
                            .await?;
                        upload_progress.show_uploaded(response_cid);

                        // Verify uploaded CID with locally computed CID
                        assert_eq!(response_cid, object_cid);
                        upload_progress.show_cid_verified();
                        upload_progress.finish();

                        // Broadcast transaction with Object's CID
                        let params = PutParams {
                            key: key.as_bytes().to_vec(),
                            kind: ObjectKind::External(object_cid),
                            overwrite: *overwrite,
                        };
                        let tx = machine
                            .put(&provider, &mut signer, params, broadcast_mode, gas_params)
                            .await?;

                        print_json(&tx)
                    } else {
                        // Handle as an internal object
                        first_chunk.truncate(first_chunk_size);
                        let tx = machine
                            .put(
                                &provider,
                                &mut signer,
                                PutParams {
                                    key: key.as_bytes().to_vec(),
                                    kind: ObjectKind::Internal(ByteBuf(first_chunk)),
                                    overwrite: *overwrite,
                                },
                                broadcast_mode,
                                gas_params,
                            )
                            .await?;

                        upload_progress.finish();
                        print_json(&tx)
                    }
                }
                Err(e) => Err(e.into()),
            }
        }
        ObjectstoreCommands::Delete(ObjectstoreDeleteArgs {
            private_key,
            key,
            address,
            broadcast_mode,
            tx_args,
        }) => {
            let TxParams {
                sequence,
                gas_params,
            } = tx_args.to_tx_params();
            let broadcast_mode = broadcast_mode.get();
            let mut signer = Wallet::new_secp256k1(
                private_key.clone(),
                AccountKind::Ethereum,
                subnet_id.clone(),
            )?;
            signer.set_sequence(sequence, &provider).await?;

            let machine = ObjectStore::attach(*address);

            let params = DeleteParams {
                key: key.as_bytes().to_vec(),
            };
            let tx = machine
                .delete(&provider, &mut signer, params, broadcast_mode, gas_params)
                .await?;

            print_json(&tx)
        }
        ObjectstoreCommands::Get(args) => {
            let machine = ObjectStore::attach(args.address);

            let key = args.key.as_str();
            let object = machine
                .get(
                    &provider,
                    GetParams {
                        key: key.as_bytes().to_vec(),
                    },
                    args.height,
                )
                .await?;

            if let Some(object) = object {
                match object {
                    Object::Internal(buf) => {
                        if let Some(range) = args.range.as_deref() {
                            let (start, end) =
                                parse_range_arg(range.to_string(), buf.0.len() as u64)?;
                            let mut stdout = io::stdout();
                            stdout
                                .write_all(&buf.0[start as usize..=end as usize])
                                .await?;
                        } else {
                            let mut stdout = io::stdout();
                            stdout.write_all(&buf.0).await?;
                        }
                        Ok(())
                    }
                    Object::External((buf, resolved)) => {
                        let cid = Cid::try_from(buf.0)?;
                        if !resolved {
                            return Err(anyhow!("object is not resolved"));
                        }

                        let object_api_url = args
                            .object_api_url
                            .clone()
                            .unwrap_or(cli.network.get().object_api_url()?);
                        let object_client = ObjectClient::new(object_api_url);

                        let progress_bar = ObjectProgressBar::new(cli.quiet);

                        // The `download` method is currently using /objectstore API
                        // since we have decided to keep the GET APIs intact for a while.
                        // If we decide to remove these APIs, we can move to Object API
                        // for downloading the file with CID.
                        machine
                            .download(
                                object_client,
                                key.to_string(),
                                args.range.clone(),
                                io::stdout(),
                            )
                            .await?;

                        progress_bar.show_downloaded(cid);
                        progress_bar.finish();

                        Ok(())
                    }
                }
            } else {
                Err(anyhow!("object not found for key '{}'", key))
            }
        }
        ObjectstoreCommands::List(args) => {
            let machine = ObjectStore::attach(args.address);

            let prefix = args.prefix.as_str();
            let delimiter = args.delimiter.as_str();
            let list = machine
                .list(
                    &provider,
                    ListParams {
                        prefix: prefix.as_bytes().to_vec(),
                        delimiter: delimiter.as_bytes().to_vec(),
                        offset: args.offset,
                        limit: args.limit,
                    },
                    args.height,
                )
                .await?;

            // TODO: ObjectList doesn't need to return as an Option. We can change this in the actor.
            let list = list.unwrap_or_default();

            let objects = list
                .objects
                .iter()
                .map(|v| {
                    let key = core::str::from_utf8(&v.0).unwrap_or_default().to_string();
                    match &v.1 {
                        ObjectListItem::Internal((cid, size)) => {
                            json!({"key": key, "value": json!({"kind": "internal", "content": cid.to_string(), "size": size})})
                        }
                        ObjectListItem::External((cid, resolved)) => {
                            json!({"key": key, "value": json!({"kind": "external", "content": cid.to_string(), "resolved": resolved})})
                        }
                    }
                })
                .collect::<Vec<Value>>();
            let common_prefixes = list
                .common_prefixes
                .iter()
                .map(|v| Value::String(core::str::from_utf8(v).unwrap_or_default().to_string()))
                .collect::<Vec<Value>>();

            print_json(&json!({"objects": objects, "common_prefixes": common_prefixes}))
        }
    }
}

// === Progress Bar ===

struct ObjectProgressBar {
    inner: Option<ProgressBar>,
}

impl ObjectProgressBar {
    fn new(quiet: bool) -> Self {
        if quiet {
            return Self { inner: None };
        }

        let inner = ProgressBar::new_spinner();
        let tick_style = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let template = "{spinner:.green} [{elapsed_precise}] {msg}";
        inner.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .tick_strings(tick_style),
        );
        inner.enable_steady_tick(std::time::Duration::from_millis(80));

        Self { inner: Some(inner) }
    }

    fn show_processing(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Processing object...", Emoji("⌛", "")));
        }
    }

    fn show_uploading(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Uploading object...", Emoji("⌛", "")));
        }
    }

    fn show_uploaded(&self, cid: Cid) {
        if let Some(bar) = &self.inner {
            bar.println(format!(
                "{}  Object uploaded (CID: {}).",
                Emoji("✅", ""),
                cid
            ));
        }
    }

    fn show_downloaded(&self, cid: Cid) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Downloaded object {}", Emoji("✅", ""), cid,));
        }
    }

    fn show_cid_verified(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Object verified.", Emoji("✅", "")));
        }
    }

    fn finish(&self) {
        if let Some(bar) = &self.inner {
            bar.finish_and_clear();
        }
    }
}
