// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Utilties for data sources

use crate::read::ReadTarget;

/// A source for values
pub trait ValueSource: ReadTarget {
    type Value: ?Sized;

    fn value(&self) -> Option<&Self::Value>;
}
