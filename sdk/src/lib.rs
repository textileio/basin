// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use adm_provider::message::GasParams;

pub mod account;
pub mod ipc;
pub mod machine;
pub mod network;

/// Arguments common to transactions.
#[derive(Clone, Default, Debug)]
pub struct TxArgs {
    /// Sender account sequence (nonce).
    pub sequence: Option<u64>,
    /// Gas params.
    pub gas_params: GasParams,
}
