// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use adm_provider::message::GasParams;
use cid::Cid;
use console::Emoji;
use indicatif::{ProgressBar, ProgressStyle};

pub mod account;
pub mod ipc;
pub mod machine;
pub mod network;

/// Arguments common to transactions.
#[derive(Clone, Default, Debug)]
pub struct TxParams {
    /// Sender account sequence (nonce).
    pub sequence: Option<u64>,
    /// Gas params.
    pub gas_params: GasParams,
}

// === Progress Bar ===

pub struct ObjectProgressBar {
    inner: Option<ProgressBar>,
}

impl ObjectProgressBar {
    pub fn new(quiet: bool) -> Self {
        if quiet {
            return Self { inner: None };
        }

        let inner = ProgressBar::new_spinner();
        let tick_style = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let template = "{spinner:.green} [{elapsed_precise}] {msg}";
        inner.set_style(
            ProgressStyle::with_template(template)
                .unwrap()
                .tick_strings(tick_style),
        );
        inner.enable_steady_tick(std::time::Duration::from_millis(80));

        Self { inner: Some(inner) }
    }

    pub fn show_processing(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Processing object...", Emoji("⌛", "")));
        }
    }

    pub fn show_uploading(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Uploading object...", Emoji("⌛", "")));
        }
    }

    pub fn show_uploaded(&self, cid: Cid) {
        if let Some(bar) = &self.inner {
            bar.println(format!(
                "{}  Object uploaded (CID: {}).",
                Emoji("✅", ""),
                cid
            ));
        }
    }

    pub fn show_downloaded(&self, cid: Cid) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Downloaded object {}", Emoji("✅", ""), cid,));
        }
    }

    pub fn show_cid_verified(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Object verified.", Emoji("✅", "")));
        }
    }

    pub fn finish(&self) {
        if let Some(bar) = &self.inner {
            bar.finish_and_clear();
        }
    }
}
