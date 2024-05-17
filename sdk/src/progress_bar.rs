// Copyright 2024 ADM Contributors
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use console::Emoji;
use indicatif::{ProgressBar, ProgressStyle};

/// A simple progress bar for object uploads and downloads.
pub struct ObjectProgressBar {
    inner: Option<ProgressBar>,
}

impl ObjectProgressBar {
    /// Create new progress bar. Use `quiet` to silence output.
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

    /// Show "processing object" text.
    pub fn show_processing(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Processing object...", Emoji("⌛", "")));
        }
    }

    /// Show "uploading object" text.
    pub fn show_uploading(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Uploading object...", Emoji("⌛", "")));
        }
    }

    /// Show uploaded object [`Cid`].
    pub fn show_uploaded(&self, cid: Cid) {
        if let Some(bar) = &self.inner {
            bar.println(format!(
                "{}  Object uploaded (CID: {}).",
                Emoji("✅", ""),
                cid
            ));
        }
    }

    /// Show downloaded object [`Cid`].
    pub fn show_downloaded(&self, cid: Cid) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Downloaded object {}", Emoji("✅", ""), cid,));
        }
    }

    /// Show "object verified" text.
    pub fn show_cid_verified(&self) {
        if let Some(bar) = &self.inner {
            bar.println(format!("{}  Object verified.", Emoji("✅", "")));
        }
    }

    /// Finish and clear the progress bar.
    pub fn finish(&self) {
        if let Some(bar) = &self.inner {
            bar.finish_and_clear();
        }
    }
}
