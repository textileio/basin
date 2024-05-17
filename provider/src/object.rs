// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use async_trait::async_trait;
use cid::Cid;
use futures_util::StreamExt;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use tendermint_rpc::Url;
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// The result of an upload.
pub struct UploadResponse {
    /// The [`Cid`] of the uploaded object.
    pub cid: Cid,
}

/// Trait implemented by object clients.
#[async_trait]
pub trait ObjectService {
    /// Upload an object to a node's Object API.
    async fn upload(
        &self,
        body: reqwest::Body,
        size: usize,
        msg: String,
        chain_id: u64,
    ) -> anyhow::Result<UploadResponse>;

    /// Download an object from a node's Object API.
    async fn download(
        &self,
        address: String,
        key: String,
        range: Option<String>,
        height: u64,
        writer: impl AsyncWrite + Unpin + Send + 'static,
    ) -> anyhow::Result<()>;
}

/// An object service client capable of uploading and downloading objects.
pub struct ObjectClient {
    inner: Client,
    endpoint: Url,
}

impl ObjectClient {
    pub fn new(endpoint: Url) -> Self {
        ObjectClient {
            inner: Client::new(),
            endpoint,
        }
    }
}

#[async_trait]
impl ObjectService for ObjectClient {
    async fn upload(
        &self,
        body: reqwest::Body,
        total_bytes: usize,
        msg: String,
        chain_id: u64,
    ) -> anyhow::Result<UploadResponse> {
        let part = Part::stream_with_length(body, total_bytes as u64)
            .file_name("upload")
            .mime_str("application/octet-stream")?;

        let form = Form::new()
            .text("chain_id", chain_id.to_string())
            .text("msg", msg)
            .part("object", part);

        let url = format!("{}v1/objects", self.endpoint);
        let response = self.inner.post(url).multipart(form).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(format!(
                "failed to upload object: {}",
                response.text().await?
            )));
        }
        let cid_str = response.text().await?;
        let cid = Cid::try_from(cid_str)?;
        Ok(UploadResponse { cid })
    }

    async fn download(
        &self,
        address: String,
        key: String,
        range: Option<String>,
        height: u64,
        mut writer: impl AsyncWrite + Unpin + Send + 'static,
    ) -> anyhow::Result<()> {
        let url = format!(
            "{}v1/objectstores/{}/{}?height={}",
            self.endpoint, address, key, height
        );
        let response = if let Some(range) = range {
            self.inner
                .get(url)
                .header("Range", format!("bytes={}", range))
                .send()
                .await?
        } else {
            self.inner.get(url).send().await?
        };
        if !response.status().is_success() {
            return Err(anyhow!(format!(
                "failed to download object: {}",
                response.text().await?
            )));
        }

        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    writer.write_all(&chunk).await?;
                }
                Err(e) => {
                    return Err(anyhow!(e));
                }
            }
        }

        Ok(())
    }
}
