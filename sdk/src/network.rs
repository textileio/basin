// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::address::{set_current_network, Network};

/// Set current network to use testnet addresses.
pub fn use_testnet_addresses() {
    set_current_network(Network::Testnet);
}
