// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Sources for typed values

use std::borrow::Borrow;
use std::time::Duration;

#[cfg(test)]
use mock_instant::global::Instant;

#[cfg(not(test))]
use std::time::Instant;

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
        if frac.is_normal() && frac >= 0.02 {
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
