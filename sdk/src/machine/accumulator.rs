// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use async_trait::async_trait;
use bytes::Bytes;
use cid::Cid;
use fendermint_actor_accumulator::Method::{Push, Root};
use fendermint_actor_machine::WriteAccess;
use fendermint_vm_actor_interface::adm::Kind;
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;

use adm_provider::{message::local_message, response::decode_cid, BroadcastMode, Provider, Tx};
use adm_signer::Signer;

use crate::machine::{deploy_machine, DeployTx, Machine};
use crate::TxArgs;

pub struct Accumulator {
    address: Address,
}

#[async_trait]
impl Machine for Accumulator {
    async fn new(
        provider: &impl Provider,
        signer: &mut impl Signer,
        write_access: WriteAccess,
        args: TxArgs,
    ) -> anyhow::Result<(Self, DeployTx)> {
        let (address, tx) =
            deploy_machine(provider, signer, Kind::Accumulator, write_access, args).await?;
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
    pub async fn push(
        &self,
        provider: &impl Provider,
        signer: &mut impl Signer,
        payload: Bytes,
        broadcast_mode: BroadcastMode,
        args: TxArgs,
    ) -> anyhow::Result<Tx<Cid>> {
        let params = RawBytes::serialize(payload.to_vec())?;
        let message = signer.transaction(
            self.address,
            Default::default(),
            Push as u64,
            params,
            None,
            args.gas_params,
        )?;
        provider.perform(message, broadcast_mode, decode_cid).await
    }

    pub async fn root(
        &self,
        provider: &impl Provider,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Cid> {
        let message = local_message(self.address, Root as u64, Default::default());
        let response = provider.call(message, height, decode_cid).await?;
        Ok(response.value)
    }
}
