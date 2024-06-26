// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use async_trait::async_trait;
use fvm_shared::address::Address;

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
    async fn download(
        &self,
        address: Address,
        key: &str,
        range: Option<String>,
        height: u64,
    ) -> anyhow::Result<reqwest::Response>;

    /// Gets the object size.
    async fn size(&self, address: Address, key: &str, height: u64) -> anyhow::Result<usize>;
}
