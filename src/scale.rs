// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Utilities for automatic scaling of units

use std::fmt;
use std::num::NonZeroU16;
use std::ops;

/// A scaled value
#[derive(Copy, Clone)]
pub struct Scaled<V, S: Scale> {
    value: V,
    scale: S,
}

impl<V, S: Scale> Scaled<V, S> {
    /// Create a new scaled value
    pub fn new(value: V, scale: S) -> Self {
        Self { value, scale }
    }

    /// Scale up while keeping the value above some minimum
    pub fn max_scale<T>(self, min_value: T) -> Self
    where
        V: ops::Div<T> + From<V::Output> + PartialOrd<T>,
        T: ops::Mul<T, Output = T> + From<u16> + Copy,
    {
        let Self {
            mut value,
            mut scale,
        } = self;
        while let Some((new_scale, factor)) = scale.step() {
            let factor = T::from(factor.get());
            if value <= factor * min_value {
                break;
            };

            value = (value / factor).into();
            scale = new_scale;
        }
        Self { value, scale }
    }
}

impl<V, S: Scale + Default> From<V> for Scaled<V, S> {
    fn from(value: V) -> Self {
        Scaled::new(value, Default::default())
    }
}

impl<V: fmt::Display, S: Scale + fmt::Display> fmt::Display for Scaled<V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Not using the `write` macro makes the first `Display::fmt` honour
        // the precision specification.
        self.value.fmt(f)?;
        self.scale.fmt(f)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    use std::num::NonZeroU8;

    #[test]
    fn max_scale_smoke() {
        let scale = DummyScale(NonZeroU8::new(2).expect("could not create dummy scale"));
        let scale = Scaled::new(2 * 3 * 4 * 5 * 6, scale).max_scale(10);
        assert_eq!(scale.to_string(), "12_5")
    }

    #[test]
    fn max_scale_exhaust() {
        let scale = DummyScale(NonZeroU8::new(253).expect("could not create dummy scale"));
        let scale = Scaled::new(254 * 255 * 256, scale).max_scale(10);
        assert_eq!(scale.to_string(), "256_255")
    }

    #[derive(Copy, Clone, PartialEq)]
    struct DummyScale(std::num::NonZeroU8);

    impl Scale for DummyScale {
        fn step(self) -> Option<(Self, NonZeroU16)> {
            self.0.checked_add(1).map(|s| (Self(s), s.into()))
        }
    }

    impl fmt::Display for DummyScale {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "_{}", self.0)
        }
    }
}
