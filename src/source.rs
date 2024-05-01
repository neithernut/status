// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Sources for typed values

use std::borrow::Borrow;

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
