// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Entries

use std::fmt;

use crate::read::Ref;
use crate::source::Source;

/// A single entry of a status line
pub trait Entry: Sized + 'static {
    /// Type being displayed for this entry
    type Display<'a>: fmt::Display + 'a
    where
        Self: 'a;

    /// Get a display representation for this entry
    fn display(&self) -> Option<Self::Display<'_>>;

    /// Transform this entry into a [Formatter]
    fn into_fmt(self) -> Formatter {
        Box::new(move |f| fmt::Display::fmt(&OptionDisplay(self.display()), f))
    }
}

impl<S> Entry for Ref<S>
where
    S: Source + 'static,
    S::Value: fmt::Display + Clone,
{
    type Display<'a> = S::Value;

    fn display(&self) -> Option<Self::Display<'_>> {
        self.borrow()
            .value()
            .as_ref()
            .map(std::borrow::Borrow::borrow)
            .cloned()
    }
}

/// Function type formatting a specific entry
pub type Formatter = Box<dyn Fn(&mut fmt::Formatter<'_>) -> fmt::Result>;

/// Entry displaying the local date and time
#[derive(Default)]
pub struct LocalTime;

impl Entry for LocalTime {
    type Display<'a> = DateTime;

    fn display(&self) -> Option<Self::Display<'_>> {
        let time = unsafe { *libc::localtime(&libc::time(std::ptr::null_mut())) };
        Some(DateTime(time))
    }
}

/// Printable date and time
pub struct DateTime(libc::tm);

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.0.tm_year + 1900,
            self.0.tm_mon,
            self.0.tm_mday,
            self.0.tm_hour,
            self.0.tm_min,
            self.0.tm_sec,
        )
    }
}

/// Helper for formatting [Option]s with [None] as `???`
struct OptionDisplay<D: fmt::Display>(pub Option<D>);

impl<D: fmt::Display> fmt::Display for OptionDisplay<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(d) = self.0.as_ref() {
            d.fmt(f)
        } else {
            f.write_str("???")
        }
    }
}
