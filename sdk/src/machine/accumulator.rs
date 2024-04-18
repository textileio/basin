// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::marker::PhantomData;

use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use cid::Cid;
use fendermint_actor_accumulator::Method::{Push, Root};
use fendermint_actor_accumulator::PushReturn;
use fendermint_actor_machine::WriteAccess;
use fendermint_vm_actor_interface::adm::Kind;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::{BytesSer, RawBytes};
use fvm_shared::address::Address;
use tendermint::abci::response::DeliverTx;
use tendermint_rpc::Client;

use adm_provider::{
    message::local_message, response::decode_bytes, response::decode_cid, BroadcastMode, Provider,
    Tx,
};
use adm_signer::Signer;

use crate::machine::{deploy_machine, DeployTx, Machine};
use crate::TxArgs;

pub struct Accumulator<C> {
    address: Address,
    _marker: PhantomData<C>,
}

#[async_trait]
impl<C> Machine<C> for Accumulator<C>
where
    C: Client + Send + Sync,
{
    async fn new(
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        write_access: WriteAccess,
        args: TxArgs,
    ) -> anyhow::Result<(Self, DeployTx)> {
        let (address, tx) =
            deploy_machine(provider, signer, Kind::Accumulator, write_access, args).await?;
        Ok((Self::attach(address), tx))
    }

    fn attach(address: Address) -> Self {
        Accumulator {
            address,
            _marker: PhantomData,
        }
    }

    fn address(&self) -> Address {
        self.address
    }
}

impl<C> Accumulator<C>
where
    C: Client + Send + Sync,
{
    pub async fn push(
        &self,
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        payload: Bytes,
        broadcast_mode: BroadcastMode,
        args: TxArgs,
    ) -> anyhow::Result<Tx<PushReturn>> {
        let params = RawBytes::serialize(BytesSer(&payload))?;
        let message = signer.transaction(
            self.address,
            Default::default(),
            Push as u64,
            params,
            None,
            args.gas_params,
        )?;
        provider
            .perform(message, broadcast_mode, decode_acc_push_return)
            .await
    }

    pub async fn root(
        &self,
        provider: &impl Provider<C>,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Cid> {
        let message = local_message(self.address, Root as u64, Default::default());
        let response = provider.call(message, height, decode_cid).await?;
        Ok(response.value)
    }
}

pub fn decode_acc_push_return(deliver_tx: &DeliverTx) -> anyhow::Result<PushReturn> {
    let data = decode_bytes(&deliver_tx)?;
    fvm_ipld_encoding::from_slice::<PushReturn>(&data)
        .map_err(|e| anyhow!("error parsing as PushReturn: {e}"))
}
