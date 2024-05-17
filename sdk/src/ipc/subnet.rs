// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use std::time::Duration;

use fvm_shared::address::Address;
use reqwest::Url;

use adm_signer::SubnetID;

/// The EVM subnet config parameters.
#[derive(Debug, Clone)]
pub struct EVMSubnet {
    pub id: SubnetID,
    pub provider_http: Url,
    pub provider_timeout: Option<Duration>,
    pub auth_token: Option<String>,
    pub registry_addr: Address,
    pub gateway_addr: Address,
}
