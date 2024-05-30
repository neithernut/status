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

    /// Prepare an [io_uring::squeue::Entry]
    ///
    /// The [io_uring::squeue::Entry] returned will read the (internally held)
    /// file into the (internally held) buffer when submitted.
    pub fn prepare(&mut self) -> io_uring::squeue::Entry {
        let fd = io_uring::types::Fd(self.file.as_raw_fd());
        let buf = Pin::into_inner(self.buf.as_mut());
        io_uring::opcode::Read::new(fd, buf.as_mut_ptr(), buf.len().try_into().unwrap()).build()
    }

    /// Process the result of a previously prepared [io_uring::squeue::Entry]
    pub fn process(&self, result: rustix::io::Result<u32>) -> Result<()> {
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
    ring: io_uring::IoUring,
    items: Vec<Item>,
}

impl Ring {
    /// Create a new ring with the given `items` using the `ring_builder`
    pub fn new(ring_builder: &io_uring::Builder, items: impl Into<Vec<Item>>) -> Result<Self> {
        let mut items = items.into();
        items.shrink_to_fit();
        let num = items
            .len()
            .max(1)
            .try_into()
            .context("Too many items for one ring")?;
        ring_builder
            .build(num)
            .context("Could not create IO uring")
            .map(|ring| Self { ring, items })
    }

    /// Prepare submission queue events for all [Item]s
    pub fn prepare(&mut self) -> Result<()> {
        let mut sq = self.ring.submission();
        self.items
            .iter_mut()
            .zip(0..)
            .map(|(t, n)| t.prepare().user_data(n))
            .try_for_each(|e| unsafe { sq.push(&e) })
            .context("Could not prepare SQEs")?;
        sq.sync();
        Ok(())
    }

    /// Submit events and dispatch completion events
    pub fn submit_and_dispatch(&mut self) -> Result<()> {
        let mut submitted = 0;
        loop {
            // We could submit and wait for all the items to complete with a
            // single call to `io_uring::IoUring::submit_and_wait`, but we want
            // to process any result as soon as possible. We therefore await a
            // single completion event. Still, we need to ensure that we process
            // all items we submitted, eventually.
            submitted = self
                .ring
                .submit_and_wait(1)
                .context("Could not submit/wait for completions")?
                .checked_add(submitted)
                .context("Accumulated too many submitted items")?;

            let mut completion = self.ring.completion();
            completion.sync();
            submitted = submitted.saturating_sub(completion.len());

            completion.try_for_each(|e| {
                let id: usize = e
                    .user_data()
                    .try_into()
                    .with_context(|| format!("Encountered invalid item id {}", e.user_data()))?;
                let result = e
                    .result()
                    .try_into()
                    .map_err(|_| rustix::io::Errno::from_raw_os_error(e.result().wrapping_neg()));
                self.items
                    .get(id)
                    .context("Could not find associated read item")?
                    .process(result)
                    .with_context(|| format!("Item with id {id} failed"))
            })?;

            if submitted == 0 {
                break Ok(());
            }
        }
    }
}

impl TryFrom<Vec<Item>> for Ring {
    type Error = anyhow::Error;

    fn try_from(items: Vec<Item>) -> Result<Self, Self::Error> {
        Self::new(&io_uring::IoUring::builder(), items)
    }
}
