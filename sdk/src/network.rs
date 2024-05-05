// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::address::{set_current_network, Network};

pub const TESTNET_SUBNET_ID: &str = "/r314159/t410fgk5jhufnidxskatxqmpd7awjb57ijettlw6g7cy";
pub const DEVNET_SUBNET_ID: &str = "test";

pub const TESTNET_RPC_URL: &str = "http://34.106.228.171:26657";
pub const DEVNET_RPC_URL: &str = "http://127.0.0.1:26657";

pub const TESTNET_PARENT_URL: &str = "https://api.calibration.node.glif.io/rpc/v1";

pub const TESTNET_PARENT_GATEWAY_ADDRESS: &str = "0x17972fF0290d8a607cd9Bd03B5c91aBe5B9E0f6E";

pub const TESTNET_PARENT_REGISTRY_ADDRESS: &str = "0x7b77B1e4E9341C7B563002E4F9836E352f18B348";

/// Set current network to use testnet addresses.
pub fn use_testnet_addresses() {
    set_current_network(Network::Testnet);
}
