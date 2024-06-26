// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use async_trait::async_trait;
use fendermint_crypto::SecretKey;
use fendermint_vm_actor_interface::eam::EthAddress;
use fendermint_vm_message::{
    chain::ChainMessage, query::FvmQueryHeight, signed::Object, signed::SignedMessage,
};
use fvm_ipld_encoding::RawBytes;
use fvm_shared::{
    address::Address, crypto::signature::Signature, econ::TokenAmount, message::Message, MethodNum,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use adm_provider::{message::GasParams, query::QueryProvider};

use crate::signer::Signer;
use crate::SubnetID;

/// Indicates how an [`Address`] should be derived from a public key.
///
/// [`AccountKind::Regular`] refers to native FVM addresses.
/// [`AccountKind::Ethereum`] refers to Ethereum style addresses.
#[derive(Debug, Clone)]
pub enum AccountKind {
    Regular,
    Ethereum,
}

/// [`Signer`] implementation that relies on a local [`SecretKey`] to sign messages.
///
/// Note, because [`Wallet`] manages the account's sequence (nonce) with a mutex,
/// using it across threads won't increase the speed at which it can sign messages.
#[derive(Debug, Clone)]
pub struct Wallet {
    addr: Address,
    sk: SecretKey,
    subnet_id: SubnetID,
    sequence: Arc<Mutex<u64>>,
}

#[async_trait]
impl Signer for Wallet {
    fn address(&self) -> Address {
        self.addr
    }

    fn secret_key(&self) -> Option<SecretKey> {
        Some(self.sk.clone())
    }

    fn subnet_id(&self) -> Option<SubnetID> {
        Some(self.subnet_id.clone())
    }

    async fn transaction(
        &mut self,
        to: Address,
        value: TokenAmount,
        method_num: MethodNum,
        params: RawBytes,
        object: Option<Object>,
        gas_params: GasParams,
    ) -> anyhow::Result<ChainMessage> {
        let mut sequence_guard = self.sequence.lock().await;
        let sequence = *sequence_guard;
        let message = Message {
            version: Default::default(),
            from: self.addr,
            to,
            sequence,
            value,
            method_num,
            params,
            gas_limit: gas_params.gas_limit,
            gas_fee_cap: gas_params.gas_fee_cap,
            gas_premium: gas_params.gas_premium,
        };
        *sequence_guard += 1;
        let signed =
            SignedMessage::new_secp256k1(message, object, &self.sk, &self.subnet_id.chain_id())?;
        Ok(ChainMessage::Signed(signed))
    }

    fn sign_message(
        &self,
        message: Message,
        object: Option<Object>,
    ) -> anyhow::Result<SignedMessage> {
        let signed =
            SignedMessage::new_secp256k1(message, object, &self.sk, &self.subnet_id.chain_id())?;
        Ok(signed)
    }

    fn verify_message(
        &self,
        message: &Message,
        object: &Option<Object>,
        signature: &Signature,
    ) -> anyhow::Result<()> {
        SignedMessage::verify_signature(message, object, signature, &self.subnet_id.chain_id())?;
        Ok(())
    }
}

impl Wallet {
    /// Returns a new secp256k1 [`Wallet`] using the given [`SecretKey`].
    ///
    /// Note, subnets only support [`AccountKind::Ethereum`].
    pub fn new_secp256k1(
        sk: SecretKey,
        kind: AccountKind,
        subnet_id: SubnetID,
    ) -> anyhow::Result<Self> {
        let pk = sk.public_key().serialize();
        let addr = match kind {
            AccountKind::Regular => Address::new_secp256k1(&pk)?,
            AccountKind::Ethereum => Address::from(EthAddress::new_secp256k1(&pk)?),
        };
        let sequence = Arc::new(Mutex::new(0));
        Ok(Wallet {
            sk,
            addr,
            subnet_id,
            sequence,
        })
    }

    /// Inititalize sequence from the actor's on-chain state.
    pub async fn init_sequence(&mut self, provider: &impl QueryProvider) -> anyhow::Result<()> {
        // Using the `Pending` state to query just in case there are other transactions initiated by the signer.
        let res = provider
            .actor_state(&self.addr, FvmQueryHeight::Pending)
            .await?;

        match res.value {
            Some((_, state)) => {
                let mut sequence_guard = self.sequence.lock().await;
                *sequence_guard = state.sequence;
                Ok(())
            }
            None => Err(anyhow!(
                "failed to init sequence; actor {} cannot be found",
                self.addr
            )),
        }
    }

    /// Set the sequence to the given value.
    /// If `maybe_sequence` is `None`, it's fetched from the actor's on-chain state.
    pub async fn set_sequence(
        &mut self,
        maybe_sequence: Option<u64>,
        provider: &impl QueryProvider,
    ) -> anyhow::Result<()> {
        if let Some(sequence) = maybe_sequence {
            let mut sequence_guard = self.sequence.lock().await;
            *sequence_guard = sequence;
        } else {
            self.init_sequence(provider).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use async_trait::async_trait;
    use fendermint_vm_message::query::{FvmQuery, FvmQueryHeight};
    use tendermint_rpc::endpoint::abci_query::AbciQuery;

    struct MockQueryProvider;

    #[async_trait]
    impl QueryProvider for MockQueryProvider {
        async fn query(
            &self,
            _query: FvmQuery,
            _height: FvmQueryHeight,
        ) -> anyhow::Result<AbciQuery> {
            // mocked query response with sequence == 65
            let response = r#"{
                "code": 0,
                "log": "",
                "info": "",
                "index": "0",
                "key": "GIM=",
                "value": "pWRjb2Rl2CpYJwABVaDkAiBuyxJuuG2pHLjnd5bBzV8iF37KrDAjVytY6h9tvJ3WT2VzdGF0ZdgqWCcAAXGg5AIgRbDPwiDO7Ft8HGLE1Bk9OOTrpI6IFXKc51+cCrDkwcBoc2VxdWVuY2UYQWdiYWxhbmNlSQB84tz/LZSh2HFkZWxlZ2F0ZWRfYWRkcmVzc1YECsBf5rY/+ks8UY5v8eWXNY7oOdsB",
                "proof": null,
                "height": "580876",
                "codespace": ""
              }"#;
            let parsed: AbciQuery = serde_json::from_str(response).unwrap();
            Ok(parsed)
        }
    }

    #[tokio::test]
    async fn test_set_sequence() {
        let mock_provider = MockQueryProvider;
        let private_key = crate::key::random_secretkey();
        let subnet_id = SubnetID::from_str("r/foobar").unwrap();
        let mut wallet =
            Wallet::new_secp256k1(private_key.clone(), AccountKind::Ethereum, subnet_id).unwrap();

        // Test setting a specific sequence value
        wallet.set_sequence(Some(50), &mock_provider).await.unwrap();
        assert_eq!(*wallet.sequence.lock().await, 50);

        // Test initializing sequence from provider
        wallet.set_sequence(None, &mock_provider).await.unwrap();
        assert_eq!(*wallet.sequence.lock().await, 65);
    }
}
