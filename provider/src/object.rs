// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use async_trait::async_trait;
use fvm_shared::address::Address;
use tokio::io::AsyncWrite;

use crate::response::Cid;

/// Provider for object interactions.
#[async_trait]
pub trait ObjectProvider: Send + Sync {
    /// Upload an object.
    async fn upload(
        &self,
        body: reqwest::Body,
        size: usize,
        msg: String,
        chain_id: u64,
    ) -> anyhow::Result<Cid>;

    /// Download an object.
    async fn download<W>(
        &self,
        address: Address,
        key: &str,
        range: Option<String>,
        height: u64,
        writer: W,
    ) -> anyhow::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static;
}
