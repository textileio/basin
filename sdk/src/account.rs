// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use ethers::prelude::TransactionReceipt;
use fendermint_vm_actor_interface::adm::{
    ListMetadataParams, Metadata, Method::ListMetadata, ADM_ACTOR_ADDR,
};
use fendermint_vm_message::query::FvmQueryHeight;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::{address::Address, econ::TokenAmount};
use tendermint::abci::response::DeliverTx;

use adm_provider::{message::local_message, response::decode_bytes, QueryProvider};
use adm_signer::Signer;

use crate::ipc::{manager::EvmManager, subnet::EVMSubnet};

/// A static wrapper around ADM account methods.
pub struct Account {}

impl Account {
    /// Get machines owned by the given [`Signer`].
    pub async fn machines(
        provider: &impl QueryProvider,
        signer: &impl Signer,
        height: FvmQueryHeight,
    ) -> anyhow::Result<Vec<Metadata>> {
        let input = ListMetadataParams {
            owner: signer.address(),
        };
        let params = RawBytes::serialize(input)?;
        let message = local_message(ADM_ACTOR_ADDR, ListMetadata as u64, params);
        let response = provider.call(message, height, decode_machines).await?;
        Ok(response.value)
    }

    /// Get the sequence (nonce) for a [`Signer`] at the given height.
    pub async fn sequence(
        provider: &impl QueryProvider,
        signer: &impl Signer,
        height: FvmQueryHeight,
    ) -> anyhow::Result<u64> {
        let response = provider.actor_state(&signer.address(), height).await?;

        match response.value {
            Some((_, state)) => Ok(state.sequence),
            None => Err(anyhow!(
                "failed to get sequence; actor {} cannot be found",
                signer.address()
            )),
        }
    }

    /// Get the balance for a [`Signer`] at the given height.
    pub async fn balance(signer: &impl Signer, subnet: EVMSubnet) -> anyhow::Result<TokenAmount> {
        EvmManager::balance(signer.address(), subnet).await
    }

    /// Deposit funds from a [`Signer`] to an address in the given subnet.
    pub async fn deposit(
        signer: &impl Signer,
        to: Address,
        subnet: EVMSubnet,
        amount: TokenAmount,
    ) -> anyhow::Result<TransactionReceipt> {
        EvmManager::deposit(signer, to, subnet, amount).await
    }

    /// Withdraw funds from a [`Signer`] to an address in the given subnet.
    pub async fn withdraw(
        signer: &impl Signer,
        to: Address,
        subnet: EVMSubnet,
        amount: TokenAmount,
    ) -> anyhow::Result<TransactionReceipt> {
        EvmManager::withdraw(signer, to, subnet, amount).await
    }

    /// Transfer funds from [`Signer`] to an address in the given subnet.
    pub async fn transfer(
        signer: &impl Signer,
        to: Address,
        subnet: EVMSubnet,
        amount: TokenAmount,
    ) -> anyhow::Result<TransactionReceipt> {
        EvmManager::transfer(signer, to, subnet, amount).await
    }
}

fn decode_machines(deliver_tx: &DeliverTx) -> anyhow::Result<Vec<Metadata>> {
    let data = decode_bytes(deliver_tx)?;
    fvm_ipld_encoding::from_slice::<Vec<Metadata>>(&data)
        .map_err(|e| anyhow!("error parsing as Vec<adm::Metadata>: {e}"))
}
