// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::min;

use anyhow::anyhow;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use bytes::Bytes;
use fendermint_actor_machine::WriteAccess;
use fendermint_actor_objectstore::{
    DeleteParams, GetParams,
    Method::{DeleteObject, GetObject, ListObjects, PutObject},
    Object, ObjectKind, ObjectList, PutParams,
};
use fendermint_vm_actor_interface::adm::Kind;
use fendermint_vm_message::{query::FvmQueryHeight, signed::Object as MessageObject};
use fvm_ipld_encoding::{serde_bytes::ByteBuf, RawBytes};
use fvm_shared::address::Address;
use indicatif::HumanDuration;
use num_traits::Zero;
use tendermint::abci::response::DeliverTx;
use tendermint_rpc::{Client, Url};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt},
    time::Instant,
};
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;
use unixfs_v1::file::adder::{Chunker, FileAdder};

use adm_provider::{
    message::{local_message, object_upload_message, GasParams},
    object::ObjectClient,
    object::ObjectService,
    response::{decode_bytes, decode_cid, Cid},
    BroadcastMode, Provider, QueryProvider, TxReceipt,
};
use adm_signer::Signer;

use crate::progress::{new_message_bar, new_multi_bar, SPARKLE};
use crate::{
    machine::{deploy_machine, DeployTxReceipt, Machine},
    progress::new_progress_bar,
};

const MAX_INTERNAL_OBJECT_LENGTH: usize = 1024;

/// Object add options.
#[derive(Clone, Default, Debug)]
pub struct AddOptions {
    pub overwrite: bool,
    pub broadcast_mode: BroadcastMode,
    pub gas_params: GasParams,
    pub show_progress: bool,
}

/// Object delete options.
#[derive(Clone, Default, Debug)]
pub struct DeleteOptions {
    pub broadcast_mode: BroadcastMode,
    pub gas_params: GasParams,
}

/// Object get options.
#[derive(Clone, Default, Debug)]
pub struct GetOptions {
    pub range: Option<String>,
    pub height: FvmQueryHeight,
    pub show_progress: bool,
}

/// Object query options.
#[derive(Clone, Debug)]
pub struct QueryOptions {
    /// The prefix to filter objects by.
    pub prefix: String,
    /// The delimiter used to define object hierarchy.
    pub delimiter: String,
    /// The offset to start listing objects from.
    pub offset: u64,
    /// The maximum number of objects to list.
    pub limit: u64,
    /// Query block height.
    pub height: FvmQueryHeight,
}

impl Default for QueryOptions {
    fn default() -> Self {
        QueryOptions {
            prefix: Default::default(),
            delimiter: "/".into(),
            offset: Default::default(),
            limit: Default::default(),
            height: Default::default(),
        }
    }
}

/// Parse a range string and return start and end byte positions.
fn parse_range(range: String, size: u64) -> anyhow::Result<(u64, u64)> {
    let range: Vec<String> = range.split('-').map(|n| n.to_string()).collect();
    if range.len() != 2 {
        return Err(anyhow!("invalid range format"));
    }
    let (start, end): (u64, u64) = match (!range[0].is_empty(), !range[1].is_empty()) {
        (true, true) => (range[0].parse::<u64>()?, range[1].parse::<u64>()?),
        (true, false) => (range[0].parse::<u64>()?, size - 1),
        (false, true) => {
            let last = range[1].parse::<u64>()?;
            if last > size {
                (0, size - 1)
            } else {
                (size - last, size - 1)
            }
        }
        (false, false) => (0, size - 1),
    };
    if start > end || end >= size {
        return Err(anyhow!("invalid range"));
    }
    Ok((start, end))
}

// async fn generate_cid<S>(reader: &(impl AsyncRead + AsyncSeek + Unpin + Send + 'static)) -> anyhow::Result<(cid::Cid, usize)>
// {
//     let chunk_size = 1024 * 1024; // size-1048576
//     let mut adder = FileAdder::builder()
//         .with_chunker(Chunker::Size(chunk_size))
//         .build();
//     let mut buffer = vec![0; chunk_size];
//     let mut size: usize = 0;
//     loop {
//         match reader.read(&mut buffer).await {
//             Ok(0) => {
//                 break;
//             }
//             Ok(n) => {
//                 let (_, n) = adder.push(&buffer[..n]);
//                 size += n;
//             }
//             Err(e) => {
//                 return Err(e.into());
//             }
//         }
//     }
//     let unixfs_iterator = adder.finish();
//     let (cid, _) = unixfs_iterator
//         .last()
//         .ok_or_else(|| anyhow!("Cannot get root CID"))?;
//     let cid =
//         cid::Cid::try_from(cid.to_bytes()).map_err(|e| anyhow!("Cannot generate CID: {}", e))?;
//     Ok((cid, size))
// }

/// A machine for S3-like object storage.
pub struct ObjectStore {
    address: Address,
}

#[async_trait]
impl Machine for ObjectStore {
    const KIND: Kind = Kind::ObjectStore;

    async fn new<C>(
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        write_access: WriteAccess,
        gas_params: GasParams,
    ) -> anyhow::Result<(Self, DeployTxReceipt)>
    where
        C: Client + Send + Sync,
    {
        let (address, tx) = deploy_machine(
            provider,
            signer,
            Kind::ObjectStore,
            write_access,
            gas_params,
        )
        .await?;
        Ok((Self::attach(address), tx))
    }

    fn attach(address: Address) -> Self {
        ObjectStore { address }
    }

    fn address(&self) -> Address {
        self.address
    }
}

impl ObjectStore {
    /// Add an object into the object store.
    pub async fn add<C, R>(
        &self,
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        object_api_url: Url,
        key: &str,
        mut reader: R,
        options: AddOptions,
    ) -> anyhow::Result<TxReceipt<Cid>>
    where
        C: Client + Send + Sync,
        R: AsyncRead + AsyncSeek + Unpin + Send + 'static,
    {
        let started = Instant::now();
        let bars = new_multi_bar(!options.show_progress);
        let msg_bar = bars.add(new_message_bar());

        // Sample reader to determine what kind of object will be added
        let mut sample = vec![0; MAX_INTERNAL_OBJECT_LENGTH + 1];
        let sampled = reader.read(&mut sample).await?;
        if sampled.is_zero() {
            return Err(anyhow!("cannot add empty object"));
        }
        reader.rewind().await?;

        if sampled > MAX_INTERNAL_OBJECT_LENGTH {
            // Handle as a detached object

            // Generate object Cid
            // We do this here to avoid moving the reader
            let chunk_size = 1024 * 1024; // size-1048576
            let mut adder = FileAdder::builder()
                .with_chunker(Chunker::Size(chunk_size))
                .build();
            let mut buffer = vec![0; chunk_size];
            let mut reader_size: usize = 0;
            let mut object_size: usize = 0;

            msg_bar.set_prefix("[1/3]");
            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        reader_size += n;
                        let (leaf, n) = adder.push(&buffer[..n]);
                        for (_, (chunk_cid, _)) in leaf.enumerate() {
                            msg_bar.set_message(format!("Processed chunk: {}", chunk_cid));
                        }
                        object_size += n;
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            let unixfs_iterator = adder.finish();
            let (cid, _) = unixfs_iterator
                .last()
                .ok_or_else(|| anyhow!("cannot get root cid"))?;
            let object_cid = cid::Cid::try_from(cid.to_bytes())
                .map_err(|e| anyhow!("cannot generate cid: {}", e))?;

            // Rewind and stream for uploading
            msg_bar.set_prefix("[2/3]");
            msg_bar.set_message(format!("Uploading {} to network...", object_cid));
            let pro_bar = bars.add(new_progress_bar(reader_size));
            reader.rewind().await?;
            let mut stream = ReaderStream::new(reader);
            let async_stream = async_stream::stream! {
                let mut progress: usize = 0;
                while let Some(chunk) = stream.next().await {
                    if let Ok(chunk) = &chunk {
                        progress = min(progress + chunk.len(), object_size);
                        pro_bar.set_position(progress as u64);
                    }
                    yield chunk;
                }
                pro_bar.finish_and_clear();
            };

            // Upload Object to Object API
            let response_cid = self
                .upload(
                    signer,
                    object_api_url,
                    key,
                    async_stream,
                    object_cid,
                    object_size,
                    options.overwrite,
                )
                .await?;

            // Verify uploaded CID with locally computed CID
            assert_eq!(response_cid, object_cid);

            // Broadcast transaction with Object's CID
            msg_bar.set_prefix("[3/3]");
            msg_bar.set_message("Broadcasting transaction...");
            let params = PutParams {
                key: key.into(),
                kind: ObjectKind::External(object_cid),
                overwrite: options.overwrite,
            };
            let serialized_params = RawBytes::serialize(params.clone())?;
            let object = Some(MessageObject::new(
                params.key.clone(),
                object_cid,
                self.address,
            ));
            let message = signer
                .transaction(
                    self.address,
                    Default::default(),
                    PutObject as u64,
                    serialized_params,
                    object,
                    options.gas_params,
                )
                .await?;

            let tx = provider
                .perform(message, options.broadcast_mode, decode_cid)
                .await?;

            msg_bar.finish_and_clear();
            if options.show_progress {
                println!(
                    "{} Added detached object in {} (cid={}; size={})",
                    SPARKLE,
                    HumanDuration(started.elapsed()),
                    object_cid,
                    object_size
                );
            }

            Ok(tx)
        } else {
            // Handle as an internal object

            // Broadcast transaction with Object's CID
            msg_bar.set_prefix("[1/1]");
            msg_bar.set_message("Broadcasting transaction...");
            sample.truncate(sampled);
            let params = PutParams {
                key: key.into(),
                kind: ObjectKind::Internal(ByteBuf(sample)),
                overwrite: options.overwrite,
            };
            let serialized_params = RawBytes::serialize(params)?;
            let message = signer
                .transaction(
                    self.address,
                    Default::default(),
                    PutObject as u64,
                    serialized_params,
                    None,
                    options.gas_params,
                )
                .await?;
            let tx = provider
                .perform(message, options.broadcast_mode, decode_cid)
                .await?;

            msg_bar.finish_and_clear();
            if options.show_progress {
                println!(
                    "{} Added object in {} (size={})",
                    SPARKLE,
                    HumanDuration(started.elapsed()),
                    sampled
                );
            }

            Ok(tx)
        }
    }

    /// Uploads an object to the Object API for staging.
    #[allow(clippy::too_many_arguments)]
    async fn upload<S>(
        &self,
        signer: &mut impl Signer,
        object_api_url: Url,
        key: &str,
        stream: S,
        cid: cid::Cid,
        size: usize,
        overwrite: bool,
    ) -> anyhow::Result<cid::Cid>
    where
        S: futures_core::stream::TryStream + Send + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        let from = signer.address();
        let params = PutParams {
            key: key.into(),
            kind: ObjectKind::External(cid),
            overwrite,
        };
        let serialized_params = RawBytes::serialize(params)?;

        let message =
            object_upload_message(from, self.address, PutObject as u64, serialized_params);
        let singed_message = signer.sign_message(
            message,
            Some(MessageObject::new(key.into(), cid, self.address)),
        )?;
        let serialized_signed_message = fvm_ipld_encoding::to_vec(&singed_message)?;

        let chain_id = match signer.subnet_id() {
            Some(id) => id.chain_id(),
            None => {
                return Err(anyhow!("failed to get subnet ID from signer"));
            }
        };

        let body = reqwest::Body::wrap_stream(stream);
        let object_client = ObjectClient::new(object_api_url);
        let response = object_client
            .upload(
                body,
                size,
                general_purpose::URL_SAFE.encode(&serialized_signed_message),
                chain_id.into(),
            )
            .await?;

        Ok(response.cid)
    }

    /// Delete an object.
    pub async fn delete<C>(
        &self,
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        key: &str,
        options: DeleteOptions,
    ) -> anyhow::Result<TxReceipt<Cid>>
    where
        C: Client + Send + Sync,
    {
        let params = DeleteParams { key: key.into() };
        let params = RawBytes::serialize(params)?;
        let message = signer
            .transaction(
                self.address,
                Default::default(),
                DeleteObject as u64,
                params,
                None,
                options.gas_params,
            )
            .await?;
        provider
            .perform(message, options.broadcast_mode, decode_cid)
            .await
    }

    /// Get an object at the given key, range, and height.
    pub async fn get<W>(
        &self,
        provider: &impl QueryProvider,
        object_api_url: Url,
        key: &str,
        mut writer: W,
        options: GetOptions,
    ) -> anyhow::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let params = GetParams { key: key.into() };
        let params = RawBytes::serialize(params)?;
        let message = local_message(self.address, GetObject as u64, params);
        let response = provider.call(message, options.height, decode_get).await?;

        if let Some(object) = response.value {
            match object {
                Object::Internal(buf) => {
                    if let Some(range) = options.range {
                        let (start, end) = parse_range(range, buf.0.len() as u64)?;
                        writer
                            .write_all(&buf.0[start as usize..=end as usize])
                            .await?;
                    } else {
                        writer.write_all(&buf.0).await?;
                    }
                    Ok(())
                }
                Object::External((buf, resolved)) => {
                    let cid = cid::Cid::try_from(buf.0)?;
                    if !resolved {
                        return Err(anyhow!("object is not resolved"));
                    }
                    // The `download` method is currently using /objectstore API
                    // since we have decided to keep the GET APIs intact for a while.
                    // If we decide to remove these APIs, we can move to Object API
                    // for downloading the file with CID.
                    self.download(object_api_url, key, writer, options.clone())
                        .await?;
                    // with_progress_bar(progress_bar.as_ref(), |p| {
                    //     p.show_downloaded(cid);
                    //     p.finish();
                    // });
                    Ok(())
                }
            }
        } else {
            Err(anyhow!("object not found for key '{}'", key))
        }
    }

    /// Download an object for the given key, range, and height.
    async fn download<W>(
        &self,
        object_api_url: Url,
        key: &str,
        writer: W,
        options: GetOptions,
    ) -> anyhow::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        ObjectClient::new(object_api_url)
            .download(
                self.address,
                key,
                options.range,
                options.height.into(),
                writer,
            )
            .await?;
        Ok(())
    }

    /// Query for objects with params at the given height.
    ///
    /// Use [`QueryOptions`] for filtering and pagination.
    pub async fn query(
        &self,
        provider: &impl QueryProvider,
        options: QueryOptions,
    ) -> anyhow::Result<Option<ObjectList>> {
        let params = fendermint_actor_objectstore::ListParams {
            prefix: options.prefix.into(),
            delimiter: options.delimiter.into(),
            offset: options.offset,
            limit: options.limit,
        };
        let params = RawBytes::serialize(params)?;
        let message = local_message(self.address, ListObjects as u64, params);
        let response = provider.call(message, options.height, decode_list).await?;
        Ok(response.value)
    }
}

fn decode_get(deliver_tx: &DeliverTx) -> anyhow::Result<Option<Object>> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice::<Option<Object>>(&data)
        .map_err(|e| anyhow!("error parsing as Option<Object>: {e}"))
}

fn decode_list(deliver_tx: &DeliverTx) -> anyhow::Result<Option<ObjectList>> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice::<Option<ObjectList>>(&data)
        .map_err(|e| anyhow!("error parsing as Option<ObjectList>: {e}"))
}

// #[cfg(test)]
// mod tests {
//     use std::io::Error;
//     use std::pin::Pin;
//     use std::task::{Context, Poll};
//
//     use rand::{thread_rng, Rng};
//     use tokio::io::AsyncRead;
//     use tokio::io::ReadBuf;
//     use tokio::sync::mpsc;
//
//     use super::*;
//
//     struct MockReader {
//         content: Vec<u8>,
//         pos: usize,
//     }
//
//     impl MockReader {
//         fn new(content: Vec<u8>) -> Self {
//             MockReader { content, pos: 0 }
//         }
//     }
//
//     impl AsyncRead for MockReader {
//         fn poll_read(
//             mut self: Pin<&mut Self>,
//             _cx: &mut Context<'_>,
//             buf: &mut ReadBuf<'_>,
//         ) -> Poll<Result<(), Error>> {
//             if self.pos >= self.content.len() {
//                 return Poll::Ready(Ok(()));
//             }
//             let max_len = buf.remaining();
//             let data_len = self.content.len() - self.pos;
//             let len_to_copy = std::cmp::min(max_len, data_len);
//
//             buf.put_slice(&self.content[self.pos..self.pos + len_to_copy]);
//             self.pos += len_to_copy;
//
//             Poll::Ready(Ok(()))
//         }
//     }
//
//     #[tokio::test]
//     async fn test_process_object() {
//         let mut rng = thread_rng();
//         let mut reader_content = vec![0u8; 1024];
//         rng.fill(&mut reader_content[..]);
//
//         let mut reader = MockReader::new(reader_content.clone());
//         let (tx, mut rx) = mpsc::channel(10);
//
//         // Read first 1024 bytes from the reader an assign it to first chunk
//         let mut first_chunk = vec![0u8; 1024];
//         reader
//             .read_exact(&mut first_chunk)
//             .await
//             .expect("Failed to read the first chunk");
//
//         let (cid, total) = process_object(reader, tx, first_chunk.clone())
//             .await
//             .unwrap();
//
//         // Initialize an empty vector to hold the chunks received
//         // from object processor
//         let mut sent_chunks = Vec::new();
//         while let Some(chunk) = rx.recv().await {
//             sent_chunks.push(chunk);
//         }
//
//         // Verify total bytes_read (first_chunk + remaining)
//         assert_eq!(total + first_chunk.len(), 1024);
//
//         // Verify CID calculation was correct by hashing the reader
//         let mut tmp = TempFile::new().await.unwrap();
//         tmp.write_all(&reader_content).await.unwrap();
//         tmp.flush().await.unwrap();
//         tmp.rewind().await.unwrap();
//         let chunk_size = 1024 * 1024;
//         let mut adder = FileAdder::builder()
//             .with_chunker(Chunker::Size(chunk_size))
//             .build();
//         let mut buffer = vec![0; chunk_size];
//         loop {
//             match tmp.read(&mut buffer).await {
//                 Ok(0) => {
//                     break;
//                 }
//                 Ok(n) => {
//                     let _ = adder.push(&buffer[..n]);
//                 }
//                 Err(e) => {
//                     panic!("Error reading from temp file: {}", e);
//                 }
//             }
//         }
//         let unixfs_iterator = adder.finish();
//         let (expected_cid, _) = unixfs_iterator.last().unwrap();
//         let expected_cid = cid::Cid::try_from(expected_cid.to_bytes()).unwrap();
//
//         assert_eq!(expected_cid, cid);
//     }
// }
