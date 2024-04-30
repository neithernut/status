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

/// Abstraction of an IO uring processing a set of [Item]s multiple times
pub struct Ring {
    ring: iou::IoUring,
    items: Vec<Item>,
}

impl Ring {
    /// Prepare submission queue events for all [Item]s
    pub fn prepare(&mut self) -> Result<()> {
        let sqes = self
            .len()
            .try_into()
            .context("Could not prepare enough SQEs")?;
        let mut sq = self.ring.sq();
        let sqes = sq
            .prepare_sqes(sqes)
            .context("Could not get submission queue events for preparation")?;
        self.items
            .iter_mut()
            .zip(sqes)
            .zip(0..)
            .for_each(|((t, mut e), n)| unsafe {
                t.prepare(&mut e);
                e.set_user_data(n);
            });
        Ok(())
    }

    /// Submit events and dispatch completion events
    pub fn submit_and_dispatch(&mut self) -> Result<()> {
        let submitted = self
            .ring
            .submit_sqes()
            .context("Could not submit read items")?;
        for _ in 0..submitted {
            let cqe = self
                .ring
                .wait_for_cqe()
                .context("Could not get read item result")?;
            let id: usize = cqe
                .user_data()
                .try_into()
                .with_context(|| format!("Encountered invalid item id {}", cqe.user_data()))?;
            self.items
                .get(id)
                .context("Could not find associated read item")?
                .process(cqe.result())
                .context("Item with id {id} failed")?;
        }
        Ok(())
    }

    /// Retrieve the number of [Item]s
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

impl TryFrom<Vec<Item>> for Ring {
    type Error = anyhow::Error;

    fn try_from(mut items: Vec<Item>) -> Result<Self, Self::Error> {
        items.shrink_to_fit();
        let num = items
            .len()
            .max(1)
            .try_into()
            .context("Too many items for one ring")?;
        let ring = iou::IoUring::new(num).context("Could not create IO uring")?;
        Ok(Self { ring, items })
    }
}
