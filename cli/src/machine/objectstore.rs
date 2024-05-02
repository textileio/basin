// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use cid::Cid;
use clap::{Args, Parser, Subcommand};
use clap_stdin::FileOrStdin;
use console::Emoji;
use fendermint_actor_machine::WriteAccess;
use fendermint_actor_objectstore::{GetParams, ListParams, Object, ObjectKind, PutParams};
use fendermint_vm_core::chainid;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::serde_bytes::ByteBuf;
use fvm_shared::address::Address;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::json;
use tendermint_rpc::HttpClient;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use adm_provider::{json_rpc::JsonRpcProvider, object::ObjectClient, BroadcastMode};
use adm_sdk::machine::{objectstore, objectstore::ObjectStore, Machine};

use crate::{get_signer, parse_address, print_json, Cli};

const MAX_INTERNAL_OBJECT_LENGTH: u64 = 1024;

#[derive(Clone, Debug, Args)]
pub struct ObjectstoreArgs {
    #[command(subcommand)]
    command: ObjectstoreCommands,
}

#[derive(Clone, Debug, Subcommand)]
enum ObjectstoreCommands {
    /// Create a new object store
    Create(ObjectstoreCreateArgs),
    /// Put an object into the object store
    Put(ObjectstorePutArgs),
    /// Get an object from the object store
    Get(ObjectstoreGetArgs),
    /// List objects in the object store
    List(ObjectstoreListArgs),
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreCreateArgs {
    /// Allow public write access to the object store
    #[arg(long, default_value_t = false)]
    public_write: bool,
}

#[derive(Clone, Debug, Parser)]
struct ObjectstorePutArgs {
    /// Address of the object store        
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Key of the object to put
    #[arg(short, long)]
    key: String,
    /// Overwrite the object if it already exists
    #[arg(short, long, action)]
    overwrite: bool,
    /// Input file path to upload
    #[clap(default_value = "-")]
    input: FileOrStdin,
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreGetArgs {
    /// Address of the object store    
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Key of the object to get
    #[arg(short, long)]
    key: String,
    /// Output file path for download
    #[arg(short, long)]
    output: String,
}

#[derive(Clone, Debug, Args)]
struct ObjectstoreListArgs {
    /// Address of the object store
    #[arg(short, long, value_parser = parse_address)]
    address: Address,
    /// Prefix to filter objects
    #[arg(short, long, default_value = "")]
    prefix: String,
    /// Delimiter to filter objects
    #[arg(short, long, default_value = "/")]
    delimiter: String,
    /// Offset to start listing objects
    #[arg(short, long, default_value_t = 0)]
    offset: u64,
    /// Limit to list objects
    #[arg(short, long, default_value_t = 0)]
    limit: u64,
}

pub async fn handle_objectstore(cli: Cli, args: &ObjectstoreArgs) -> anyhow::Result<()> {
    let provider = JsonRpcProvider::new_http(cli.rpc_url, None)?;
    let chain_id = chainid::from_str_hashed(&cli.chain_name)?;
    let object_client = ObjectClient::new(cli.object_api_url, u64::from(chain_id));

    match &args.command {
        ObjectstoreCommands::Create(args) => {
            let mut signer = get_signer(&provider, cli.wallet_pk, cli.chain_name).await?;
            let write_access = if args.public_write {
                WriteAccess::Public
            } else {
                WriteAccess::OnlyOwner
            };
            let (store, tx) =
                ObjectStore::new(&provider, &mut signer, write_access, Default::default()).await?;

            print_json(&json!({"address": store.address().to_string(), "tx": &tx}))
        }
        ObjectstoreCommands::Put(ObjectstorePutArgs {
            key,
            address,
            overwrite,
            input,
        }) => {
            let mut signer =
                get_signer(&provider, cli.wallet_pk.clone(), cli.chain_name.clone()).await?;
            let machine = ObjectStore::<HttpClient>::attach(*address);
            let mut reader = input.into_async_reader().await?;
            let mut first_chunk = vec![0; MAX_INTERNAL_OBJECT_LENGTH as usize];
            let upload_progress = ObjectProgressBar::new();

            match reader.read_exact(&mut first_chunk).await {
                Ok(first_chunk_size) => {
                    let (tx, rx) = mpsc::channel::<Vec<u8>>(1024);
                    // Preprocess Object before uploading
                    upload_progress.show_processing();
                    let (object_cid, bytes_read) =
                        objectstore::process_object(reader, tx, first_chunk).await?;

                    // Upload Object with signature
                    upload_progress.show_uploading();
                    let params = PutParams {
                        key: key.as_bytes().to_vec(),
                        kind: ObjectKind::External(object_cid),
                        overwrite: *overwrite,
                    };
                    let response_cid = machine
                        .upload(
                            &mut signer,
                            object_client,
                            key.clone(),
                            object_cid,
                            rx,
                            first_chunk_size as usize + bytes_read,
                            params.clone(),
                        )
                        .await?;
                    upload_progress.show_uploaded(response_cid.clone());

                    // Verify uploaded CID with locally computed CID
                    assert!(response_cid == object_cid);
                    upload_progress.show_cid_verified();
                    upload_progress.finish();

                    // Broadcast transaction with Object's CID
                    let tx = machine
                        .put(
                            &provider,
                            &mut signer,
                            params,
                            BroadcastMode::Commit,
                            Default::default(),
                        )
                        .await?;
                    print_json(&tx)?;
                }
                Err(e) => {
                    // internal object
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        reader.read_to_end(&mut first_chunk).await?;

                        let tx = machine
                            .put(
                                &provider,
                                &mut signer,
                                PutParams {
                                    key: key.as_bytes().to_vec(),
                                    kind: ObjectKind::Internal(ByteBuf(first_chunk)),
                                    overwrite: *overwrite,
                                },
                                BroadcastMode::Commit,
                                Default::default(),
                            )
                            .await?;

                        print_json(&tx)?;
                    } else {
                        return Err(e.into());
                    }
                }
            }
            Ok(())
        }
        ObjectstoreCommands::Get(args) => {
            // TODO: Handle range requests
            let machine = ObjectStore::<HttpClient>::attach(args.address);
            let key = args.key.as_str();
            let object = machine
                .get(
                    &provider,
                    GetParams {
                        key: key.as_bytes().to_vec(),
                    },
                    FvmQueryHeight::Committed,
                )
                .await?;

            if let Some(object) = object {
                match object {
                    Object::Internal(buf) => {
                        let mut stdout = io::stdout();
                        stdout.write_all(&buf.0).await?;
                        Ok(())
                    }
                    Object::External((buf, resolved)) => {
                        let cid = Cid::try_from(buf.0)?;
                        if !resolved {
                            return Err(anyhow!("object is not resolved"));
                        }

                        let progress_bar = ObjectProgressBar::new();

                        // The `download` method is currently using /objectstore API
                        // since we have decided to keep the GET APIs intact for a while.
                        // If we decide to remove these APIs we can move to Object API
                        // for downloading the file with CID.
                        let file = File::create(args.output.clone()).await?;
                        machine
                            .download(object_client, key.to_string(), file)
                            .await?;

                        progress_bar.show_downloaded(cid, args.output.clone());
                        progress_bar.finish();

                        Ok(())
                    }
                }
            } else {
                Err(anyhow!("object not found for key '{}'", key))
            }
        }
        ObjectstoreCommands::List(args) => {
            let machine = ObjectStore::<HttpClient>::attach(args.address);
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
                    FvmQueryHeight::Committed,
                )
                .await?;
            // TODO: ObjectList doesn't need to return as an Option. We can change this in the actor.

            if let Some(list) = list {
                print_json(&list)
            } else {
                Err(anyhow!("object list not found for prefix '{}'", prefix))
            }
        }
    }
}

// === Progress Bar ===

struct ObjectProgressBar {
    inner: ProgressBar,
}

impl ObjectProgressBar {
    fn new() -> Self {
        let inner = ProgressBar::new_spinner();
        let tick_style = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let template = "{spinner:.green} [{elapsed_precise}] {msg}";
        inner.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .tick_strings(tick_style),
        );
        inner.enable_steady_tick(std::time::Duration::from_millis(80));

        Self { inner }
    }

    fn show_processing(&self) {
        self.inner
            .println(format!("{}Processing object...", Emoji("🏗️  ", ""),));
    }

    fn show_uploading(&self) {
        self.inner
            .println(format!("{}Uploading object...", Emoji("📡  ", ""),));
    }

    fn show_uploaded(&self, cid: Cid) {
        self.inner
            .println(format!("{}Upload complete {}", Emoji("✔️  ", ""), cid));
    }

    fn show_downloaded(&self, cid: Cid, path: String) {
        self.inner.println(format!(
            "{}Downloaded object {} at {}",
            Emoji("✔️  ", ""),
            cid,
            path
        ));
    }

    fn show_cid_verified(&self) {
        self.inner
            .println(format!("{}CID verified...", Emoji("✅  ", ""),));
    }

    fn finish(&self) {
        self.inner.finish_and_clear();
    }
}
