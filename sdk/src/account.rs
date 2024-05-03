// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use fendermint_vm_actor_interface::adm::{
    ListMetadataParams, Metadata, Method::ListMetadata, ADM_ACTOR_ADDR,
};
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use tendermint::abci::response::DeliverTx;
use tendermint_rpc::Client;

use adm_provider::{message::local_message, response::decode_bytes, Provider};

pub struct Account {
    pub address: Address,
}

impl Account {
    pub async fn machines<C>(
        provider: &impl Provider<C>,
        owner: Address,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Vec<Metadata>>
    where
        C: Client + Send + Sync,
    {
        let input = ListMetadataParams { owner };
        let params = RawBytes::serialize(input)?;
        let message = local_message(ADM_ACTOR_ADDR, ListMetadata as u64, params);
        let response = provider.call(message, height, decode_machines).await?;
        Ok(response.value)
    }
}

fn decode_machines(deliver_tx: &DeliverTx) -> anyhow::Result<Vec<Metadata>> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice::<Vec<Metadata>>(&data)
        .map_err(|e| anyhow!("error parsing as Vec<adm::Metadata>: {e}"))
}
