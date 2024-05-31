// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Entries

use std::fmt;
use std::ops::{Div, Mul};

use crate::read::Ref;
use crate::scale;
use crate::source::Source;

/// A single entry of a status line
pub trait Entry: Sized + 'static {
    /// Type being displayed for this entry
    type Display<'a>: fmt::Display + 'a
    where
        Self: 'a;

    /// Get a display representation for this entry
    fn display(&self) -> Option<Self::Display<'_>>;

    /// Transform this entry into a labeled one
    fn with_label<L>(self, label: L) -> Labeled<L, Self>
    where
        L: fmt::Display + Sized + 'static,
    {
        Labeled { label, entry: self }
    }

    /// Transform this entry into one with a specific precision
    fn with_precision(self, precision: u8) -> Precision<Self> {
        Precision {
            entry: self,
            precision,
        }
    }

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

impl Entry for Option<&'static str> {
    type Display<'a> = &'a str;

    fn display(&self) -> Option<Self::Display<'_>> {
        *self
    }
}

impl Entry for Option<f32> {
    type Display<'a> = f32;

    fn display(&self) -> Option<Self::Display<'_>> {
        *self
    }
}

/// Function type formatting a specific entry
pub type Formatter = Box<dyn Fn(&mut fmt::Formatter<'_>) -> fmt::Result>;

/// [fmt::Display] for displaying a space-separated list of entries
pub struct EntriesDisplay(Vec<Formatter>);

impl From<Vec<Formatter>> for EntriesDisplay {
    fn from(formatters: Vec<Formatter>) -> Self {
        Self(formatters)
    }
}

impl fmt::Display for EntriesDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries = self.0.iter();
        let Some(first) = entries.next() else {
            return Ok(());
        };

        first(f)?;
        entries.try_for_each(|e| {
            f.write_str(" ")?;
            e(f)
        })
    }
}

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
            self.0.tm_mon + 1,
            self.0.tm_mday,
            self.0.tm_hour,
            self.0.tm_min,
            self.0.tm_sec,
        )
    }
}

/// A labeled [Entry]
pub struct Labeled<L: fmt::Display + Sized + 'static, E: Entry> {
    label: L,
    entry: E,
}

impl<L: fmt::Display + Sized + 'static, E: Entry> Entry for Labeled<L, E> {
    type Display<'a> = LabeledDisplay<'a, L, E::Display<'a>>;

    fn display(&self) -> Option<Self::Display<'_>> {
        Some(Self::Display {
            label: &self.label,
            display: self.entry.display(),
        })
    }
}

/// A labeled [fmt::Display]
pub struct LabeledDisplay<'l, L: fmt::Display, D: fmt::Display> {
    label: &'l L,
    display: Option<D>,
}

impl<L: fmt::Display, D: fmt::Display> fmt::Display for LabeledDisplay<'_, L, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display = OptionDisplay(self.display.as_ref());
        write!(f, "{}: {}", self.label, display)
    }
}

/// An [Entry] with a specified precision
pub struct Precision<E: Entry> {
    entry: E,
    precision: u8,
}

impl<E: Entry> Entry for Precision<E> {
    type Display<'a> = PrecisionDisplay<E::Display<'a>>;

    fn display(&self) -> Option<Self::Display<'_>> {
        self.entry.display().map(|d| Self::Display {
            display: d,
            precision: self.precision,
        })
    }
}

/// A [fmt::Display] with a specified precision
pub struct PrecisionDisplay<D: fmt::Display> {
    display: D,
    precision: u8,
}

impl<D: fmt::Display> fmt::Display for PrecisionDisplay<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{0:.1$}", self.display, self.precision.into())
    }
}

/// An [Entry] scaling a value
pub struct AutoScaled<E: Entry, S: scale::Scale, T> {
    entry: E,
    scale: S,
    min_value: T,
}

impl<E, S, T> Entry for AutoScaled<E, S, T>
where
    E: Entry,
    for<'a> E::Display<'a>: Div<T> + From<<E::Display<'a> as Div<T>>::Output> + PartialOrd<T>,
    S: scale::Scale + fmt::Display + 'static,
    T: Mul<T, Output = T> + From<u16> + Copy + 'static,
{
    type Display<'a> = scale::Scaled<E::Display<'a>, S>;

    fn display(&self) -> Option<Self::Display<'_>> {
        self.entry
            .display()
            .map(|d| Self::Display::new(d, self.scale).max_scale(self.min_value))
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::f32::consts::PI;

    #[test]
    fn entry_display_smoke() {
        let entries: EntriesDisplay = vec![
            Some("a").into_fmt(),
            Some("bc").into_fmt(),
            None::<&str>.into_fmt(),
            Some("def").into_fmt(),
        ]
        .into();
        assert_eq!(entries.to_string(), "a bc ??? def")
    }

    #[test]
    fn entry_display_empty() {
        let entries: EntriesDisplay = Vec::new().into();
        assert_eq!(entries.to_string(), "")
    }

    #[test]
    fn label_smoke() {
        let entries: EntriesDisplay = vec![Some("a").with_label("val").into_fmt()].into();
        assert_eq!(entries.to_string(), "val: a")
    }

    #[test]
    fn label_empty() {
        let entries: EntriesDisplay = vec![None::<&str>.with_label("val").into_fmt()].into();
        assert_eq!(entries.to_string(), "val: ???")
    }

    #[test]
    fn precision_0() {
        let entries: EntriesDisplay = vec![Some(PI).with_precision(0).into_fmt()].into();
        assert_eq!(entries.to_string(), "3")
    }

    #[test]
    fn precision_1() {
        let entries: EntriesDisplay = vec![Some(PI).with_precision(1).into_fmt()].into();
        assert_eq!(entries.to_string(), "3.1")
    }

    #[test]
    fn precision_2() {
        let entries: EntriesDisplay = vec![Some(PI).with_precision(2).into_fmt()].into();
        assert_eq!(entries.to_string(), "3.14")
    }

    #[test]
    fn precision_none() {
        let entries: EntriesDisplay = vec![None::<f32>.with_precision(2).into_fmt()].into();
        assert_eq!(entries.to_string(), "???")
    }
}
