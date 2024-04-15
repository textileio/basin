// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use fendermint_vm_message::{chain::ChainMessage, signed::Object};
use fvm_ipld_encoding::RawBytes;
use fvm_shared::{address::Address, econ::TokenAmount, MethodNum};

use adm_provider::message::GasParams;

pub trait Signer: Clone + Send + Sync {
    fn address(&self) -> Address;

    fn transaction(
        &mut self,
        to: Address,
        value: TokenAmount,
        method_num: MethodNum,
        params: RawBytes,
        object: Option<Object>,
        gas_params: GasParams,
    ) -> anyhow::Result<ChainMessage>;
}
