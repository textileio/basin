// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use fendermint_actor_accumulator::Method::{Count, Get, Peaks, Push, Root};
use fendermint_actor_machine::WriteAccess;
use fendermint_vm_actor_interface::adm::Kind;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::{BytesSer, RawBytes};
use fvm_shared::address::Address;
use serde::{Deserialize, Serialize};
use tendermint::abci::response::DeliverTx;
use tendermint_rpc::Client;

use adm_provider::{
    message::local_message, message::GasParams, response::decode_bytes, response::decode_cid,
    response::Cid, BroadcastMode, Provider, QueryProvider, TxReceipt,
};
use adm_signer::Signer;

use crate::machine::{deploy_machine, DeployTx, Machine};

const MAX_ACC_PAYLOAD_SIZE: usize = 1024 * 500;

/// Display-friendly version of [`fendermint_actor_accumulator::PushReturn`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushReturn {
    pub root: Cid,
    pub index: u64,
}

impl From<fendermint_actor_accumulator::PushReturn> for PushReturn {
    fn from(v: fendermint_actor_accumulator::PushReturn) -> Self {
        Self {
            root: v.root.into(),
            index: v.index,
        }
    }
}

/// A machine for event stream accumulation.
pub struct Accumulator {
    address: Address,
}

#[async_trait]
impl Machine for Accumulator {
    async fn new<C>(
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        write_access: WriteAccess,
        gas_params: GasParams,
    ) -> anyhow::Result<(Self, DeployTx)>
    where
        C: Client + Send + Sync,
    {
        let (address, tx) = deploy_machine(
            provider,
            signer,
            Kind::Accumulator,
            write_access,
            gas_params,
        )
        .await?;
        Ok((Self::attach(address), tx))
    }

    fn attach(address: Address) -> Self {
        Accumulator { address }
    }

    fn address(&self) -> Address {
        self.address
    }
}

impl Accumulator {
    /// Push a payload into the accumulator.
    pub async fn push<C>(
        &self,
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        payload: Bytes,
        broadcast_mode: BroadcastMode,
        gas_params: GasParams,
    ) -> anyhow::Result<TxReceipt<PushReturn>>
    where
        C: Client + Send + Sync,
    {
        if payload.len() > MAX_ACC_PAYLOAD_SIZE {
            return Err(anyhow!(
                "max payload size is {} bytes",
                MAX_ACC_PAYLOAD_SIZE
            ));
        }

        let params = RawBytes::serialize(BytesSer(&payload))?;
        let message = signer
            .transaction(
                self.address,
                Default::default(),
                Push as u64,
                params,
                None,
                gas_params,
            )
            .await?;
        provider
            .perform(message, broadcast_mode, decode_push_return)
            .await
    }

    /// Get leaf stored at a given index and height.
    pub async fn leaf(
        &self,
        provider: &impl QueryProvider,
        index: u64,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Vec<u8>> {
        let params = RawBytes::serialize(index)?;
        let message = local_message(self.address, Get as u64, params);
        let response = provider
            .call(message, height, |tx| decode_leaf(tx, index))
            .await?;
        Ok(response.value)
    }

    /// Get total leaf count at a given height.
    pub async fn count(
        &self,
        provider: &impl QueryProvider,
        height: FvmQueryHeight,
    ) -> anyhow::Result<u64> {
        let message = local_message(self.address, Count as u64, Default::default());
        let response = provider.call(message, height, decode_count).await?;
        Ok(response.value)
    }

    /// Get all peaks at a given height.
    pub async fn peaks(
        &self,
        provider: &impl QueryProvider,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Vec<Cid>> {
        let message = local_message(self.address, Peaks as u64, Default::default());
        let response = provider.call(message, height, decode_peaks).await?;
        Ok(response.value)
    }

    /// Get the root at a given height.
    pub async fn root(
        &self,
        provider: &impl QueryProvider,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Cid> {
        let message = local_message(self.address, Root as u64, Default::default());
        let response = provider.call(message, height, decode_cid).await?;
        Ok(response.value)
    }
}

fn decode_push_return(deliver_tx: &DeliverTx) -> anyhow::Result<PushReturn> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice::<fendermint_actor_accumulator::PushReturn>(&data)
        .map(|r| r.into())
        .map_err(|e| anyhow!("error parsing as PushReturn: {e}"))
}

fn decode_leaf(deliver_tx: &DeliverTx, index: u64) -> anyhow::Result<Vec<u8>> {
    let data = decode_bytes(deliver_tx)?;
    if data.is_empty() {
        // TODO: The actor's leaf method should return an optional.
        return Err(anyhow!("leaf not found at index '{}'", index));
    }
    fvm_ipld_encoding::from_slice(&data).map_err(|e| anyhow!("error parsing as Vec<u8>: {e}"))
}

fn decode_count(deliver_tx: &DeliverTx) -> anyhow::Result<u64> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice(&data).map_err(|e| anyhow!("error parsing as u64: {e}"))
}

fn decode_peaks(deliver_tx: &DeliverTx) -> anyhow::Result<Vec<Cid>> {
    let data = decode_bytes(deliver_tx)?;
    let items = fvm_ipld_encoding::from_slice::<Vec<cid::Cid>>(&data)
        .map_err(|e| anyhow!("error parsing as Vec<Cid>: {e}"))?;
    let mut mapped: Vec<Cid> = vec![];
    for i in items {
        mapped.push(i.into());
    }
    Ok(mapped)
}
