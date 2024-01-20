// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Read abstractions

use std::mem::MaybeUninit;
use std::rc::Rc;

use anyhow::{Context, Result};
use io_uring::{cqueue, opcode, squeue};
use rustix::{fd, io, time};

/// One second as an [rtime::Timespec]
const TIMESPEC_SECOND: time::Timespec = time::Timespec {
    tv_sec: 1,
    tv_nsec: 0,
};

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

pub struct TimerFd {
    fd: fd::OwnedFd,
    buf: [MaybeUninit<u8>; 8],
}

impl TimerFd {
    pub fn new() -> io::Result<Self> {
        time::timerfd_create(
            time::TimerfdClockId::Realtime,
            time::TimerfdFlags::CLOEXEC,
        ).map(|fd| Self {fd, buf: [MaybeUninit::uninit(); 8]})
    }

    pub fn arm_timer(&self) -> io::Result<()> {
        use time::TimerfdTimerFlags as TimerFlags;

        loop {
            let mut current = time::clock_gettime(time::ClockId::Realtime);
            current.tv_nsec = 0;

            const FLAGS: TimerFlags = TimerFlags::union(TimerFlags::ABSTIME, TimerFlags::CANCEL_ON_SET);
            let spec = time::Itimerspec {
                it_interval: TIMESPEC_SECOND,
                it_value: current,
            };
            match time::timerfd_settime(&self.fd, FLAGS, &spec) {
                Ok(_) => return Ok(()),
                Err(io::Errno::CANCELED) => (),
                Err(e) => return Err(e),
            }
        }
    }
}

impl ReadTarget for TimerFd {
    fn read(&self) -> opcode::Read {
        use rustix::fd::AsRawFd;

        let fd = io_uring::types::Fd(self.fd.as_raw_fd());

        // We do have two rather nasty casts in here...
        opcode::Read::new(fd, self.buf.as_ptr() as _, self.buf.len() as u32)
    }

    fn update(&self, len: usize) -> Result<()> {
        Ok(())
    }

    fn process(&self, result: i32) -> Result<()> {
        if result > 0 {
            Ok(())
        } else {
            match io::Errno::from_raw_os_error(result.wrapping_neg()) {
                io::Errno::CANCELED => self.arm_timer().context("Could not rearm timer"),
                e => Err(anyhow::Error::new(e).context("Broken timer")),
            }
        }
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

