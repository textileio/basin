// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use ethers::prelude::TransactionReceipt;
use fendermint_vm_actor_interface::adm::{
    ListMetadataParams, Metadata, Method::ListMetadata, ADM_ACTOR_ADDR,
};
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::{address::Address, econ::TokenAmount, METHOD_SEND};
use ipc_provider::config::subnet::EVMSubnet;
use tendermint::abci::response::DeliverTx;
use tendermint_rpc::Client;

use adm_provider::{message::local_message, response::decode_bytes, BroadcastMode, Provider, Tx};
use adm_signer::Signer;

use crate::ipc::EvmManager;
use crate::TxArgs;

pub struct Account {}

impl Account {
    pub async fn machines<C>(
        provider: &impl Provider<C>,
        signer: &impl Signer,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Vec<Metadata>>
    where
        C: Client + Send + Sync,
    {
        let input = ListMetadataParams {
            owner: signer.address(),
        };
        let params = RawBytes::serialize(input)?;
        let message = local_message(ADM_ACTOR_ADDR, ListMetadata as u64, params);
        let response = provider.call(message, height, decode_machines).await?;
        Ok(response.value)
    }

    pub async fn deposit(
        signer: &impl Signer,
        to: Address,
        subnet: EVMSubnet,
        amount: TokenAmount,
    ) -> anyhow::Result<TransactionReceipt> {
        let manager = EvmManager::new(signer, subnet)?;
        manager.deposit(to, amount).await
    }

    pub async fn withdraw(
        signer: &impl Signer,
        to: Address,
        subnet: EVMSubnet,
        amount: TokenAmount,
    ) -> anyhow::Result<TransactionReceipt> {
        let manager = EvmManager::new(signer, subnet)?;
        manager.withdraw(to, amount).await
    }

    pub async fn transfer<C>(
        provider: &impl Provider<C>,
        signer: &mut impl Signer,
        to: Address,
        amount: TokenAmount,
        args: TxArgs,
    ) -> anyhow::Result<Tx<()>>
    where
        C: Client + Send + Sync,
    {
        let message = signer.transaction(
            to,
            amount,
            METHOD_SEND,
            RawBytes::default(),
            None,
            args.gas_params,
        )?;
        provider
            .perform(message, BroadcastMode::Commit, |_| Ok(()))
            .await
    }
}

fn decode_machines(deliver_tx: &DeliverTx) -> anyhow::Result<Vec<Metadata>> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice::<Vec<Metadata>>(&data)
        .map_err(|e| anyhow!("error parsing as Vec<adm::Metadata>: {e}"))
}
