// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use adm_provider::message::GasParams;
use anyhow::anyhow;

pub mod account;
pub mod ipc;
pub mod machine;
pub mod network;
pub mod progress_bar;

/// Arguments common to transactions.
#[derive(Clone, Default, Debug)]
pub struct TxParams {
    /// Sender account sequence (nonce).
    pub sequence: Option<u64>,
    /// Gas params.
    pub gas_params: GasParams,
}

/// Parse range CLI argument and return start and end byte positions.
fn parse_range_arg(range: String, size: u64) -> anyhow::Result<(u64, u64)> {
    let range: Vec<String> = range.split('-').map(|n| n.to_string()).collect();
    if range.len() != 2 {
        return Err(anyhow!("invalid range format"));
    }
    let (start, end): (u64, u64) = match (!range[0].is_empty(), !range[1].is_empty()) {
        (true, true) => (range[0].parse::<u64>()?, range[1].parse::<u64>()?),
        (true, false) => (range[0].parse::<u64>()?, size - 1),
        (false, true) => {
            let last = range[1].parse::<u64>()?;
            if last > size {
                (0, size - 1)
            } else {
                (size - last, size - 1)
            }
        }
        (false, false) => (0, size - 1),
    };
    if start > end || end >= size {
        return Err(anyhow!("invalid range"));
    }
    Ok((start, end))
}
