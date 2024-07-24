// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Sources for typed values

use std::borrow::Borrow;
use std::time::Duration;

use crate::Instant;

/// A source for values
pub trait Source {
    /// Type of value this source provides
    type Value;

    /// Type through which the value is provided
    type Borrow<'a>: Borrow<Self::Value> = Self::Value
    where
        Self: 'a;

    /// Retrieve the (current) value from this source
    fn value(&self) -> Option<Self::Borrow<'_>>;

    /// Make a [Gated] [Source] with the given predicate
    fn gated_with<C>(self, condition: C) -> Gated<Self, C>
    where
        C: Fn() -> bool,
        Self: Sized,
    {
        Gated::new(self, condition)
    }
}

impl<T: Clone> Source for Option<T> {
    type Value = T;

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

/// Something (usually a [Source]) that requests some sort of processing
pub trait WantsProcessing {
    /// Determine whether an processing is wanted before the given [Instant]
    ///
    /// Whether processing is wanted or not also affects whether any input
    /// values need to be supplied for that processing.
    fn wants_processing(&self, _before: Instant) -> bool {
        true
    }
}

impl<T> WantsProcessing for Option<T> {}

/// A [Source] that should be updated with a "lower" rate
///
/// This [Source] does accept every update it receives and serves that value.
/// However, it [WantsProcessing] only if a specific duration has passed since
/// the last update.
pub struct LowerRate<T> {
    data: Option<(T, Instant)>,
    rate: Duration,
}

impl<T> LowerRate<T> {
    /// Create a new [Source] wanting updates only once in the given [Duration]
    pub fn new(rate: Duration) -> Self {
        Self { data: None, rate }
    }
}

impl<T: Clone> Source for LowerRate<T> {
    type Value = T;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.data.clone().map(|(v, _)| v)
    }
}

impl<T> Updateable for LowerRate<T> {
    type Value = T;

    fn update(&mut self, value: Self::Value) {
        self.data = Some((value, Instant::now()));
    }

    fn update_invalid(&mut self) {
        self.data = None;
    }
}

impl<T> WantsProcessing for LowerRate<T> {
    fn wants_processing(&self, before: Instant) -> bool {
        self.data
            .as_ref()
            .map(|(_, l)| before.duration_since(*l) >= self.rate)
            .unwrap_or(true)
    }
}

/// A moving average
///
/// This [Source] will yield an average over all values with which it was
/// updated over a defined timespan.
pub struct MovingAverage<T = f32> {
    current: Option<(T, Instant)>,
    span: Duration,
}

impl<T> MovingAverage<T> {
    /// Create a new moving average spanning the given [Duration]
    pub fn new(span: Duration) -> Self {
        Self {
            current: None,
            span,
        }
    }

    /// Minimum fraction of the timespan required for an update to be accepted
    const MIN_UPDATE_FRAC: f32 = 0.02;
}

impl<T: Clone> Source for MovingAverage<T> {
    type Value = T;

    type Borrow<'a> = Self::Value where Self::Value: 'a;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.current.as_ref().map(|(v, _)| v.clone())
    }
}

impl<T> Updateable for MovingAverage<T>
where
    T: std::ops::Add<Output = T> + std::ops::Mul<f32, Output = T> + Copy + Default,
{
    type Value = T;

    fn update(&mut self, value: Self::Value) {
        let now = Instant::now();
        let Some((last_avg, then)) = self.current else {
            self.current = Some((value, now));
            return;
        };

        let duration = now.duration_since(then);
        if duration >= self.span {
            // Just comparing durations lets us avoid fractions greater than
            // `1.0` if we didn't receive valid updates for some reason.
            self.current = Some((value, now));
            return;
        }

        // We want to dodge situations in which we'll end up with a huge error,
        // and we definitely want to dodge infs and NaNs.
        let frac = duration.as_secs_f32() / self.span.as_secs_f32();
        if frac.is_normal() && frac >= Self::MIN_UPDATE_FRAC {
            self.current = Some((value * frac + last_avg * (1. - frac), now));
        }
    }

    fn update_invalid(&mut self) {
        // We're fine with a stale states as long as we have values from within
        // the timespan we average over.
        if self
            .current
            .map(|(_, t)| t.elapsed() > self.span)
            .unwrap_or(true)
        {
            self.current = None;
        }
    }
}

impl<T> WantsProcessing for MovingAverage<T> {
    fn wants_processing(&self, before: Instant) -> bool {
        self.current
            .as_ref()
            .map(|(_, l)| before.duration_since(*l) >= self.span.mul_f32(Self::MIN_UPDATE_FRAC))
            .unwrap_or(true)
    }
}

/// An entity that wants processing depending on a condition
pub struct Gated<S, C: Fn() -> bool> {
    inner: S,
    condition: C,
}

impl<S, C: Fn() -> bool> Gated<S, C> {
    /// Create a new gated entity
    pub fn new(source: S, condition: C) -> Self {
        Self {
            inner: source,
            condition,
        }
    }
}

impl<S: Source, C: Fn() -> bool> Source for Gated<S, C> {
    type Value = <S as Source>::Value;

    type Borrow<'a> = S::Borrow<'a> where Self: 'a;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        self.inner.value()
    }
}

impl<S: Updateable, C: Fn() -> bool> Updateable for Gated<S, C> {
    type Value = <S as Updateable>::Value;

    fn update(&mut self, value: Self::Value) {
        self.inner.update(value)
    }
}

impl<S: WantsProcessing, C: Fn() -> bool> WantsProcessing for Gated<S, C> {
    fn wants_processing(&self, before: Instant) -> bool {
        (self.condition)() && self.inner.wants_processing(before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::f32::consts::PI;

    use mock_instant::global::MockClock;

    #[test]
    fn moving_average_no_update() {
        let avg = MovingAverage::<f32>::new(Duration::from_secs(5));
        assert_eq!(avg.value(), None);
    }

    #[test]
    fn moving_average_single_update() {
        let mut avg = MovingAverage::<f32>::new(Duration::from_secs(5));
        avg.update(PI);
        assert_eq!(avg.value(), Some(PI));
    }

    #[test]
    fn moving_average_early_update() {
        let mut avg = MovingAverage::<f32>::new(Duration::from_secs(300));
        avg.update(PI);
        MockClock::advance(Duration::from_secs(5));
        avg.update(0.);
        assert_eq!(avg.value(), Some(PI));
    }

    #[test]
    fn moving_average_late_update() {
        let mut avg = MovingAverage::<f32>::new(Duration::from_secs(300));
        avg.update(PI);
        MockClock::advance(Duration::from_secs(600));
        avg.update(0.);
        assert_eq!(avg.value(), Some(0.));
    }

    #[test]
    fn moving_average_timely_update() {
        let mut avg = MovingAverage::<f32>::new(Duration::from_secs(300));
        avg.update(PI);
        MockClock::advance(Duration::from_secs(60));
        avg.update(0.);
        let value = avg.value().expect("No average availible");
        assert!(value > 0.);
        assert!(value < PI);
    }

    #[test]
    fn moving_average_retain_valid() {
        let mut avg = MovingAverage::<f32>::new(Duration::from_secs(300));
        avg.update(PI);
        MockClock::advance(Duration::from_secs(60));
        avg.update_invalid();
        assert_eq!(avg.value(), Some(PI));
    }

    #[test]
    fn moving_average_flush_valid() {
        let mut avg = MovingAverage::<f32>::new(Duration::from_secs(300));
        avg.update(PI);
        MockClock::advance(Duration::from_secs(600));
        avg.update_invalid();
        assert_eq!(avg.value(), None);
    }
}
