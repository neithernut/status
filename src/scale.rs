// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Utilities for automatic scaling of units

use std::num::NonZeroU16;

/// Trait for scales
///
/// This trait allows abstracting over (unit) scales, such as SI prefixes. Types
/// implementing this trait will usually be enums encoding some prefix or unit.
pub trait Scale: Copy {
    /// Get the next larger scale of this series
    ///
    /// This function returns the next scale in the series, with the conversion
    /// factor of the current item to the next one. If the current scale is
    /// already the largest one, this function returns `None`.
    fn step(self) -> Option<(Self, NonZeroU16)>;
}
