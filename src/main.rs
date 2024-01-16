// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2023, 2024

mod entry;
mod read;
mod source;

use anyhow::{Context, Error, Result};
use rustix::{io::Errno, time as rtime};

/// Format for datetimes
const DATETIME_FORMAT: &[time::format_description::FormatItem] =
    time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

/// One second as an [rtime::Timespec]
const TIMESPEC_SECOND: rtime::Timespec = rtime::Timespec {
    tv_sec: 1,
    tv_nsec: 0,
};

/// Arm a timer to tick on exact wallclock seconds
fn arm_timer(timer: impl rustix::fd::AsFd + Copy) -> Result<()> {
    use rtime::TimerfdTimerFlags as TimerFlags;

    loop {
        let mut current = rtime::clock_gettime(rtime::ClockId::Realtime);
        current.tv_nsec = 0;

        const FLAGS: TimerFlags = TimerFlags::union(TimerFlags::ABSTIME, TimerFlags::CANCEL_ON_SET);
        let spec = rtime::Itimerspec {
            it_interval: TIMESPEC_SECOND,
            it_value: current,
        };
        match rtime::timerfd_settime(timer, FLAGS, &spec) {
            Ok(_) => return Ok(()),
            Err(Errno::CANCELED) => (),
            Err(e) => return Err(Error::new(e).context("Could not arm timer")),
        }
    }
}

fn main() -> Result<()> {
    use std::io::Write;

    let mut entries: Vec<Box<dyn std::fmt::Display>> = Default::default();
    let mut reads = read::Reads::new();

    // TODO: add entries

    let timer = rtime::timerfd_create(
        rtime::TimerfdClockId::Realtime,
        rtime::TimerfdFlags::CLOEXEC,
    )
    .context("Could not create timer")?;
    arm_timer(&timer)?;

    let mut ring = io_uring::IoUring::new(reads.len().max(1).try_into().unwrap_or(u32::MAX))
        .context("Could not creatio IO uring")?;

    let mut entry_buffer: Vec<io_uring::squeue::Entry> = Default::default();
    let mut output_buffer: Vec<u8> = Default::default();
    loop {
        // We re-use these buffers in order to avoid repeated allocations. Which
        // means we need to clear it manually.
        entry_buffer.clear();
        output_buffer.clear();

        {
            entry_buffer.extend(reads.uring_entries());
            let mut submission = ring.submission();
            unsafe { submission.push_multiple(entry_buffer.as_ref()) }
                .context("Could not push some read items")?;
        }

        match rustix::io::read_uninit(&timer, &mut [core::mem::MaybeUninit::uninit(); 8]) {
            Ok(_) => (),
            Err(Errno::CANCELED) => arm_timer(&timer)?,
            Err(e) => return Err(Error::new(e).context("Broken timer")),
        };

        ring.submit().context("Could not submit read items")?;
        ring.completion().try_for_each(|e| reads.process(e))?;

        time::OffsetDateTime::now_local()
            .context("Could not get current time")?
            .format_into(&mut output_buffer, DATETIME_FORMAT)
            .context("Could not format current time")?;

        entries.iter().try_for_each(|e| write!(output_buffer, " {}", e))
            .context("Could not format entry")?;

        output_buffer
            .write_all(b"\n")
            .context("Could not finalize line")?;
        std::io::stdout()
            .write_all(output_buffer.as_ref())
            .context("Could not print status line")?;
    }
}
