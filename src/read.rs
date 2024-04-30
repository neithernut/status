// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Read abstractions

use std::fs::File;
use std::os::fd::AsRawFd;
use std::pin::Pin;

use anyhow::{Context, Result};

/// Processor for buffer contents
///
/// Buffer processors are meant for constructing read [Item]s. They receive the
/// read contents via [BufProcessor::process].
pub trait BufProcessor {
    fn process(&mut self, buf: &[u8]);
}

/// Convenience type for [std::rc::Rc]s wrapping [std::cell::RefCell]
pub type Ref<E> = std::rc::Rc<std::cell::RefCell<E>>;

/// A (recurring) read of a [File]
///
/// A read item represents a read of a [File] into a buffer of fixed size, from
/// the file's beginning. The read contents is then passed to a [BufProcessor].
pub struct Item {
    file: File,
    buf: Pin<Box<[u8]>>,
    extract: Ref<dyn BufProcessor>,
}

impl Item {
    /// Create a new read item
    pub fn new(file: File, buf_size: usize, extract: Ref<impl BufProcessor + 'static>) -> Self {
        let mut buf = Vec::new();
        buf.resize(buf_size, b'0');
        Self {
            file,
            buf: Pin::new(buf.into_boxed_slice()),
            extract,
        }
    }

    /// Prepare an [iou::SQE]
    ///
    /// The [iou::SQE] will repared such that it will read the (internally held)
    /// file into the (internally held) buffer.
    pub unsafe fn prepare(&mut self, sqe: &mut iou::SQE) {
        sqe.prep_read(self.file.as_raw_fd(), Pin::into_inner(self.buf.as_mut()), 0)
    }

    /// Process the result of a previously prepared [iou::SQE]
    pub fn process(&self, result: std::io::Result<u32>) -> Result<()> {
        let length = result
            .context("Operation failed")?
            .try_into()
            .unwrap_or(usize::MAX);
        anyhow::ensure!(
            length <= self.buf.len(),
            "Read length ({length}) exceeds buffer size ({})",
            self.buf.len(),
        );

        self.extract.borrow_mut().process(&self.buf[..length]);
        Ok(())
    }
}
