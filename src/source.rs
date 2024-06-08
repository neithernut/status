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

impl<T: Clone> Source for Option<T> {
    type Value = T;

    type Borrow<'a> = Self::Value where Self::Value: 'a;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.clone()
    }
}

/// Something (usually a [Source]) that can be updated "directly"
pub trait Updateable {
    /// Type with which this can be updated
    type Value;

    /// Update with a new (valid) value
    fn update(&mut self, value: Self::Value);

    /// Signal the presence of an invalid value
    fn update_invalid(&mut self) {}
}

impl<T> Updateable for Option<T> {
    type Value = T;

    fn update(&mut self, value: Self::Value) {
        *self = Some(value);
    }

    fn update_invalid(&mut self) {
        *self = None
    }
}
