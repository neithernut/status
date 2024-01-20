// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Read abstractions

use anyhow::Result;
use io_uring::opcode;
use rustix::io;

/// A target for read
pub trait ReadTarget {
    fn read(&self) -> opcode::Read;

    fn update(&self, len: usize) -> Result<()>;

    fn process(&self, result: i32) -> Result<()> {
        let length = result
            .try_into()
            .map_err(|_| io::Errno::from_raw_os_error(result.wrapping_neg()))?;
        self.update(length)
    }
}

