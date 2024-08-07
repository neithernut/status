// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Read abstractions

use std::fs::File;
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::source::{self, WantsProcessing};
use crate::Instant;

/// Processor for buffer contents
///
/// Buffer processors are meant for constructing read [Item]s. They receive the
/// read contents via [BufProcessor::process].
pub trait BufProcessor: WantsProcessing {
    fn process(&mut self, buf: &[u8]);
}

/// [BufProcessor] extracting a single (parsed) substring
pub struct Simple<U> {
    source: U,
    split_fn: fn(&u8) -> bool,
}

impl<U> Simple<U> {
    /// Create a new simple [BufProcessor]
    pub fn new(source: U, split_fn: fn(&u8) -> bool) -> Self {
        Self { source, split_fn }
    }

    pub fn new_default(split_fn: fn(&u8) -> bool) -> Self
    where
        U: Default,
    {
        Self::new(Default::default(), split_fn)
    }
}

impl<U> BufProcessor for Simple<U>
where
    U: source::Updateable + WantsProcessing,
    U::Value: FromStr,
{
    fn process(&mut self, buf: &[u8]) {
        let data = buf
            .split(self.split_fn)
            .find(|w| !w.is_empty())
            .and_then(|w| std::str::from_utf8(w).ok())
            .and_then(|s| s.parse().ok());
        if let Some(data) = data {
            self.source.update(data)
        } else {
            self.source.update_invalid()
        }
    }
}

impl<U: source::Source> source::Source for Simple<U> {
    type Value = U::Value;

    type Borrow<'a> = U::Borrow<'a> where U: 'a;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.source.value()
    }
}

impl<U: WantsProcessing> WantsProcessing for Simple<U> {
    fn wants_processing(&self, before: Instant) -> bool {
        self.source.wants_processing(before)
    }
}

/// [BufProcessor] for a 10min average PSI info
#[derive(Default)]
pub struct PSI<U> {
    source: U,
}

impl<U> From<U> for PSI<U> {
    fn from(source: U) -> Self {
        Self { source }
    }
}

impl<U> BufProcessor for PSI<U>
where
    U: source::Updateable<Value = f32> + WantsProcessing,
{
    fn process(&mut self, buf: &[u8]) {
        let data = buf
            .split(|c| *c == b'\n')
            .filter_map(|l| l.strip_prefix(b"some"))
            .flat_map(|l| l.split(u8::is_ascii_whitespace))
            .filter_map(|w| w.strip_prefix(b"avg10="))
            .find_map(|w| std::str::from_utf8(w).ok())
            .and_then(|s| s.parse().ok());
        if let Some(data) = data {
            self.source.update(data)
        } else {
            self.source.update_invalid()
        }
    }
}

impl<U: source::Source> source::Source for PSI<U> {
    type Value = U::Value;

    type Borrow<'a> = U::Borrow<'a> where U: 'a;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.source.value()
    }
}

impl<U: WantsProcessing> WantsProcessing for PSI<U> {
    fn wants_processing(&self, before: Instant) -> bool {
        self.source.wants_processing(before)
    }
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
        let fd = io_uring::types::Fd(self.raw_fd());
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

    /// Retrieve the file descriptor for this item
    pub fn raw_fd(&self) -> std::os::fd::RawFd {
        self.file.as_raw_fd()
    }
}

impl WantsProcessing for Item {
    fn wants_processing(&self, before: Instant) -> bool {
        self.extract.borrow().wants_processing(before)
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

        let ring = ring_builder
            .build(num)
            .context("Could not create IO uring")?;

        let fds: Vec<_> = items.iter().map(Item::raw_fd).collect();
        ring.submitter()
            .register_files(fds.as_ref())
            .context("Could not register fds")?;

        Ok(Self { ring, items })
    }

    /// Prepare submission queue events for all [Item]s
    pub fn prepare(&mut self) -> Result<()> {
        // We expect the next processing to be roughly in a second from now. We
        // include some additional time to account for any jitter.
        let estimated_processing = Instant::now() + Duration::from_millis(1100);

        let mut sq = self.ring.submission();
        self.items
            .iter_mut()
            .zip(0..)
            .filter(|(t, _)| t.wants_processing(estimated_processing))
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

#[cfg(test)]
mod tests {
    use super::*;

    use source::Source;

    #[test]
    fn simple_single() {
        let mut processor = Simple::<Option<u64>>::new_default(u8::is_ascii_whitespace);
        processor.process(b"123");
        assert_eq!(processor.value(), Some(123));
    }

    #[test]
    fn simple_multiple() {
        let mut processor = Simple::<Option<u64>>::new_default(u8::is_ascii_whitespace);
        processor.process(b"123 456");
        assert_eq!(processor.value(), Some(123));
    }

    #[test]
    fn simple_invalid_single() {
        let mut processor = Simple::<Option<u64>>::new_default(u8::is_ascii_whitespace);
        processor.process(b"foo");
        assert_eq!(processor.value(), None);
    }

    #[test]
    fn simple_invalid_multiple() {
        let mut processor = Simple::<Option<u64>>::new_default(u8::is_ascii_whitespace);
        processor.process(b"foo 123");
        assert_eq!(processor.value(), None);
    }

    #[test]
    fn psi_smoke() {
        let buf = concat!(
            "some avg10=1.23 avg60=4.56 avg300=7.89 total=123456\n",
            "full avg10=2.34 avg60=5.67 avg300=8.90 total=789123\n",
        );
        let mut processor = PSI::from(None);
        processor.process(buf.as_ref());
        assert_eq!(processor.value(), Some(1.23));
    }
}
