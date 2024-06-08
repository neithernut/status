// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024

use anyhow::{Context, Error, Result};
use rustix::{io::Errno, time};

mod entry;
mod read;
mod scale;
mod source;
mod spec;

fn main() -> Result<()> {
    use std::io::Write;

    let mut reads: Vec<read::Item> = Default::default();
    let entries: entry::EntriesDisplay = spec::entries(&mut reads)
        .context("Could not parse entry specifications")?
        .into();
    let mut builder = io_uring::IoUring::builder();
    builder
        .setup_submit_all()
        .setup_coop_taskrun()
        .setup_single_issuer();
    let mut ring = read::Ring::new(&builder, reads)?;

    // Timer ticking on wallclock seconds
    let timer = time::timerfd_create(time::TimerfdClockId::Realtime, time::TimerfdFlags::CLOEXEC)
        .context("Could not create timer")?;
    arm_timer(&timer)?;

    let mut output_buffer: Vec<u8> = Default::default();
    loop {
        // We re-use the buffer in order to avoid repeated allocations. Which
        // means we need to clear it manually.
        output_buffer.clear();

        ring.prepare().context("Could not prepare read items")?;

        match rustix::io::read_uninit(&timer, &mut [core::mem::MaybeUninit::uninit(); 8]) {
            Ok(_) => (),
            Err(Errno::CANCELED) => arm_timer(&timer)?,
            Err(e) => return Err(Error::new(e).context("Broken timer")),
        };
        ring.submit_and_dispatch()
            .context("Could not dispatch read items")?;

        writeln!(output_buffer, "{entries}").context("Could not format line")?;
        std::io::stdout()
            .write_all(output_buffer.as_ref())
            .context("Could not print status line")?;
    }
}

/// One second as a [time::Timespec]
const TIMESPEC_SECOND: time::Timespec = time::Timespec {
    tv_sec: 1,
    tv_nsec: 0,
};

/// Arm a timer to tick on exact wallclock seconds
fn arm_timer(timer: impl rustix::fd::AsFd + Copy) -> Result<()> {
    use time::TimerfdTimerFlags as TimerFlags;

    loop {
        let mut current = time::clock_gettime(time::ClockId::Realtime);
        current.tv_nsec = 0;

        const FLAGS: TimerFlags = TimerFlags::union(TimerFlags::ABSTIME, TimerFlags::CANCEL_ON_SET);
        let spec = time::Itimerspec {
            it_interval: TIMESPEC_SECOND,
            it_value: current,
        };
        match time::timerfd_settime(timer, FLAGS, &spec) {
            Ok(_) => return Ok(()),
            Err(Errno::CANCELED) => (),
            Err(e) => return Err(Error::new(e).context("Could not arm timer")),
        }
    }
}
