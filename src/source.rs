// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Sources for typed values

use std::borrow::Borrow;
use std::str::FromStr;

use crate::read::BufProcessor;

/// A source for values
pub trait Source {
    /// Type of value this source provides
    type Value;

    /// Type through which the value is provided
    type Borrow<'a>: Borrow<Self::Value>
    where
        Self: 'a;

    /// Retrieve the (current) value from this source
    fn value(&self) -> Option<Self::Borrow<'_>>;
}

/// Source for a single (parsed) word extracted from a buffer
#[derive(Default)]
pub struct Word<T: FromStr + Clone> {
    data: Option<T>,
}

impl<T: FromStr + Clone> Source for Word<T> {
    type Value = T;

    type Borrow<'a> = Self::Value where Self::Value: 'a;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.data.clone()
    }
}

impl<T: FromStr + Clone> BufProcessor for Word<T> {
    fn process(&mut self, buf: &[u8]) {
        self.data = buf
            .split(u8::is_ascii_whitespace)
            .find(|w| !w.is_empty())
            .and_then(|w| std::str::from_utf8(w).ok())
            .and_then(|s| s.parse().ok());
    }
}

/// Source for a 10min average PSI info
#[derive(Default)]
pub struct PSI {
    data: Option<f32>,
}

impl Source for PSI {
    type Value = f32;

    type Borrow<'a> = Self::Value;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.data
    }
}

impl BufProcessor for PSI {
    fn process(&mut self, buf: &[u8]) {
        self.data = buf
            .split(|c| *c == b'\n')
            .filter_map(|l| l.strip_prefix(b"some"))
            .flat_map(|l| l.split(u8::is_ascii_whitespace))
            .filter_map(|w| w.strip_prefix(b"avg10="))
            .find_map(|w| std::str::from_utf8(w).ok())
            .and_then(|s| s.parse().ok());
    }
}
