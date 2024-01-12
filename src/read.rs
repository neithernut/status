// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Read abstractions

use std::rc::Rc;

use anyhow::{Context, Result};
use io_uring::{cqueue, opcode, squeue};
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

#[derive(Default)]
pub struct Reads {
    data: Vec<(Rc<dyn ReadTarget>, squeue::Flags)>,
}

impl Reads {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn register(&mut self, source: Rc<impl ReadTarget + 'static>) {
        self.register_with_flags(source, squeue::Flags::ASYNC)
    }

    pub fn register_with_flags(&mut self, source: Rc<impl ReadTarget + 'static>, flags: squeue::Flags) {
        self.data.push((source, flags))
    }

    pub fn uring_entries(&self) -> impl Iterator<Item = squeue::Entry> + '_ {
        self.data
            .iter()
            .zip(0..)
            .map(|((t, f), n)| t.read().build().flags(*f).user_data(n))
    }

    pub fn process(&self, entry: cqueue::Entry) -> Result<()> {
        let read: usize = entry.user_data().try_into().context("Could not find associated read item")?;
        self.data
            .get(read)
            .context("Could not find associated read item")?
            .0
            .process(entry.result())
            .context("Read item failed")
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

