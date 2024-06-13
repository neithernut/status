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

    /// Transform this entry into one with a unit
    fn with_unit<U>(self, unit: U) -> WithUnit<Self, U>
    where
        U: fmt::Display + Sized + 'static,
    {
        WithUnit { entry: self, unit }
    }

    /// Transform this entry into one with a specific precision
    fn with_precision(self, precision: u8) -> Precision<Self> {
        Precision {
            entry: self,
            precision,
        }
    }

    /// Transform this entry into one with automatic scaling
    fn autoscaled<V, S: scale::Scale>(self, min_value: V, scale: S) -> AutoScaled<Self, S, V> {
        AutoScaled {
            entry: self,
            scale,
            min_value,
        }
    }

    /// Transform this entry into a [fmt::Display]
    fn into_fmt(self) -> Box<dyn fmt::Display> {
        use fmt::Display;

        Box::new(FormatterFn(move |f| OptionDisplay(self.display()).fmt(f)))
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

impl<F: Fn() -> Option<D> + 'static, D: fmt::Display + 'static> Entry for F {
    type Display<'a> = D;

    fn display(&self) -> Option<Self::Display<'_>> {
        self()
    }
}

impl Entry for Option<&'static str> {
    type Display<'a> = &'a str;

    fn display(&self) -> Option<Self::Display<'_>> {
        *self
    }
}

impl Entry for Option<u32> {
    type Display<'a> = u32;

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

/// Create an [Entry] mapping a [Source]
pub fn mapped<S, F, D>(source: Ref<S>, func: F) -> impl for<'a> Entry<Display<'a> = D>
where
    S: Source + 'static,
    F: Fn(&S::Value) -> Option<D> + 'static,
    D: fmt::Display + 'static,
{
    move || {
        source
            .borrow()
            .value()
            .as_ref()
            .map(std::borrow::Borrow::borrow)
            .and_then(&func)
    }
}

/// Create an [Entry] zipping two [Source]s
pub fn zipped<S1, S2, F, D>(
    source1: Ref<S1>,
    source2: Ref<S2>,
    func: F,
) -> impl for<'a> Entry<Display<'a> = D>
where
    S1: Source + 'static,
    S2: Source + 'static,
    F: Fn(&S1::Value, &S2::Value) -> Option<D> + 'static,
    D: fmt::Display + 'static,
{
    use std::borrow::Borrow;
    use std::cell::RefCell;

    move || {
        Option::zip(
            RefCell::borrow(&source1).value(),
            RefCell::borrow(&source2).value(),
        )
        .and_then(|(v1, v2)| func(v1.borrow(), v2.borrow()))
    }
}

/// Utility for formatting a "formatting `Fn`"
struct FormatterFn<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result>(F);

impl<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result> fmt::Display for FormatterFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.0)(f)
    }
}

/// [fmt::Display] for displaying a space-separated list of entries
pub struct EntriesDisplay(Vec<Box<dyn fmt::Display>>);

impl From<Vec<Box<dyn fmt::Display>>> for EntriesDisplay {
    fn from(formatters: Vec<Box<dyn fmt::Display>>) -> Self {
        Self(formatters)
    }
}

impl fmt::Display for EntriesDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries = self.0.iter();
        let Some(first) = entries.next() else {
            return Ok(());
        };

        first.fmt(f)?;
        entries.try_for_each(|e| write!(f, " {e}"))
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

/// An [Entry] with a unit
pub struct WithUnit<E: Entry, U: fmt::Display + Sized + 'static> {
    entry: E,
    unit: U,
}

impl<E: Entry, U: fmt::Display + Sized + 'static> Entry for WithUnit<E, U> {
    type Display<'a> = WithUnitDisplay<'a, E::Display<'a>, U>;

    fn display(&self) -> Option<Self::Display<'_>> {
        self.entry.display().map(|d| Self::Display {
            display: d,
            unit: &self.unit,
        })
    }
}

/// A [fmt::Display] with a unit attached
pub struct WithUnitDisplay<'u, D: fmt::Display, U: fmt::Display> {
    display: D,
    unit: &'u U,
}

impl<D: fmt::Display, U: fmt::Display> fmt::Display for WithUnitDisplay<'_, D, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.display, self.unit)
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
#[allow(non_snake_case)]
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

    #[test]
    fn autoscaled_4ki() {
        let entries: EntriesDisplay = vec![Some(4 * 1024)
            .autoscaled(2, scale::BinScale::default())
            .into_fmt()]
        .into();
        assert_eq!(entries.to_string(), "4ki")
    }

    #[test]
    fn autoscaled_2Mi() {
        let entries: EntriesDisplay = vec![Some(2 * 1024 * 1024)
            .autoscaled(2, scale::BinScale::default())
            .into_fmt()]
        .into();
        assert_eq!(entries.to_string(), "2048ki")
    }

    #[test]
    fn autoscaled_none() {
        let entries: EntriesDisplay = vec![None::<u32>
            .autoscaled(2, scale::BinScale::default())
            .into_fmt()]
        .into();
        assert_eq!(entries.to_string(), "???")
    }

    #[test]
    fn autoscaled_piki() {
        let entries: EntriesDisplay = vec![Some(PI * 1024.)
            .autoscaled(1.5f32, scale::BinScale::default())
            .with_precision(2)
            .into_fmt()]
        .into();
        assert_eq!(entries.to_string(), "3.14ki")
    }

    #[test]
    fn with_unit_smoke() {
        let entries: EntriesDisplay = vec![Some(5).with_unit("zurakos").into_fmt()].into();
        assert_eq!(entries.to_string(), "5zurakos")
    }

    #[test]
    fn with_unit_none() {
        let entries: EntriesDisplay = vec![None::<u32>.with_unit("zurakos").into_fmt()].into();
        assert_eq!(entries.to_string(), "???")
    }
}
