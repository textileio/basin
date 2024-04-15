// Copyright 2024 ADM Contributors
// Copyright 2022-2024 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Context};
use base64::Engine;
use bytes::Bytes;
use cid::Cid;
use fvm_ipld_encoding::RawBytes;
use tendermint::abci::response::DeliverTx;

/// Apply the encoding that Tendermint does to the bytes inside [`DeliverTx`].
pub(crate) fn encode_data(data: &[u8]) -> Bytes {
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    let bz = b64.as_bytes();
    Bytes::copy_from_slice(bz)
}

/// Parse what Tendermint returns in the `data` field of [`DeliverTx`] into bytes.
/// Somewhere along the way it replaces them with the bytes of a Base64 encoded string,
/// and `tendermint_rpc` does not undo that wrapping.
pub(crate) fn decode_data(data: &Bytes) -> anyhow::Result<RawBytes> {
    let b64 = String::from_utf8(data.to_vec()).context("error parsing data as base64 string")?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("error parsing base64 to bytes")?;
    Ok(RawBytes::from(data))
}

/// Parse what Tendermint returns in the `data` field of [`DeliverTx`] as raw bytes.
///
/// Only call this after the `code` of both [`DeliverTx`] and [`CheckTx`] have been inspected!
pub fn decode_bytes(deliver_tx: &DeliverTx) -> anyhow::Result<RawBytes> {
    decode_data(&deliver_tx.data)
}

/// Parse what Tendermint returns in the `data` field of [`DeliverTx`] as a [`Cid`].
pub fn decode_cid(deliver_tx: &DeliverTx) -> anyhow::Result<Cid> {
    let data = decode_data(&deliver_tx.data)?;
    fvm_ipld_encoding::from_slice::<Cid>(&data).map_err(|e| anyhow!("error parsing as Cid: {e}"))
}
