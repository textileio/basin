// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::str::FromStr;

use anyhow::anyhow;
use fvm_shared::address::{set_current_network, Address, Network as FvmNetwork};
use tendermint_rpc::Url;

use adm_provider::util::parse_address;
use adm_signer::SubnetID;

const TESTNET_SUBNET_ID: &str = "/r314159/t410f7x4mh62k6oymmd3rfdjnzyjid7p2tstnbuvnc4i";
const LOCALNET_SUBNET_ID: &str = "/r314159/t410f726d2jv6uj4mpkcbgg5ndlpp3l7dd5rlcpgzkoi";
const DEVNET_SUBNET_ID: &str = "test";

const TESTNET_RPC_URL: &str = "https://api.n1.testnet.basin.storage";
const LOCALNET_RPC_URL: &str = "http://127.0.0.1:26657";

const TESTNET_EVM_RPC_URL: &str = "https://evm-api.n1.testnet.basin.storage";
const TESTNET_EVM_GATEWAY_ADDRESS: &str = "0x77aa40b105843728088c0132e43fc44348881da8";
const TESTNET_EVM_REGISTRY_ADDRESS: &str = "0x74539671a1d2f1c8f200826baba665179f53a1b7";

const TESTNET_PARENT_EVM_RPC_URL: &str = "https://api.calibration.node.glif.io/rpc/v1";
const TESTNET_PARENT_EVM_GATEWAY_ADDRESS: &str = "0x728F3B71EBD1358973AbCE325Fe45f7f701ea7e6";
const TESTNET_PARENT_EVM_REGISTRY_ADDRESS: &str = "0x2f71A1d47ccc2E13E646D4C1bcF89E3409114De8";

const TESTNET_OBJECT_API_URL: &str = "https://object-api.n1.testnet.basin.storage";
const LOCALNET_OBJECT_API_URL: &str = "http://127.0.0.1:8001";

/// Set current network to use testnet addresses.
pub fn use_testnet_addresses() {
    set_current_network(FvmNetwork::Testnet);
}

/// Network presets for a subnet configuration and RPC URLs.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Network {
    /// Network presets for mainnet.
    Mainnet,
    /// Network presets for Calibration (default pre-mainnet).
    Testnet,
    /// Network presets for a local three-node network.
    Localnet,
    /// Network presets for local development.
    Devnet,
}

impl Network {
    /// Returns the network [`SubnetID`].
    pub fn subnet(&self) -> anyhow::Result<SubnetID> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(SubnetID::from_str(TESTNET_SUBNET_ID)?),
            Network::Localnet => Ok(SubnetID::from_str(LOCALNET_SUBNET_ID)?),
            Network::Devnet => Ok(SubnetID::from_str(DEVNET_SUBNET_ID)?),
        }
    }

    /// Returns the network [`Url`] of the CometBFT PRC API.
    pub fn rpc_url(&self) -> anyhow::Result<Url> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(Url::from_str(TESTNET_RPC_URL)?),
            Network::Localnet | Network::Devnet => Ok(Url::from_str(LOCALNET_RPC_URL)?),
        }
    }

    /// Returns the network [`Url`] of the Object API.
    pub fn object_api_url(&self) -> anyhow::Result<Url> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(Url::from_str(TESTNET_OBJECT_API_URL)?),
            Network::Localnet | Network::Devnet => Ok(Url::from_str(LOCALNET_OBJECT_API_URL)?),
        }
    }

    /// Returns the network [`reqwest::Url`] of the EVM PRC API.
    pub fn evm_rpc_url(&self) -> anyhow::Result<reqwest::Url> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(reqwest::Url::from_str(TESTNET_EVM_RPC_URL)?),
            Network::Localnet | Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    /// Returns the network [`Address`] of the EVM Gateway contract.
    pub fn evm_gateway(&self) -> anyhow::Result<Address> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(parse_address(TESTNET_EVM_GATEWAY_ADDRESS)?),
            Network::Localnet | Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    /// Returns the network [`Address`] of the EVM Registry contract.
    pub fn evm_registry(&self) -> anyhow::Result<Address> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(parse_address(TESTNET_EVM_REGISTRY_ADDRESS)?),
            Network::Localnet | Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    /// Returns the network [`reqwest::Url`] of the parent EVM PRC API.
    pub fn parent_evm_rpc_url(&self) -> anyhow::Result<reqwest::Url> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(reqwest::Url::from_str(TESTNET_PARENT_EVM_RPC_URL)?),
            Network::Localnet | Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    /// Returns the network [`Address`] of the parent EVM Gateway contract.
    pub fn parent_evm_gateway(&self) -> anyhow::Result<Address> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(parse_address(TESTNET_PARENT_EVM_GATEWAY_ADDRESS)?),
            Network::Localnet | Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }

    /// Returns the network [`Address`] of the parent EVM Registry contract.
    pub fn parent_evm_registry(&self) -> anyhow::Result<Address> {
        match self {
            Network::Mainnet => Err(anyhow!("network is pre-mainnet")),
            Network::Testnet => Ok(parse_address(TESTNET_PARENT_EVM_REGISTRY_ADDRESS)?),
            Network::Localnet | Network::Devnet => Err(anyhow!("network has no parent")),
        }
    }
}
