// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2023

use anyhow::{Context, Result};

/// Format for datetimes
const DATETIME_FORMAT: &[time::format_description::FormatItem] =
    time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

fn main() -> Result<()> {
    use std::io::Write;

    let mut output_buffer: Vec<u8> = Default::default();
    loop {
        // We re-use the buffer in order to avoid repeated allocations. Which
        // means we need to clear it manually.
        output_buffer.clear();

        time::OffsetDateTime::now_local()
            .context("Could not get current time")?
            .format_into(&mut output_buffer, DATETIME_FORMAT)
            .context("Could not format current time")?;

        output_buffer
            .write_all(b"\n")
            .context("Could not finalize line")?;
        std::io::stdout()
            .write_all(output_buffer.as_ref())
            .context("Could not print status line")?;
    }
}
