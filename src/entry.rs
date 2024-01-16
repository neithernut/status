// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Entry types

use std::borrow::Cow;
use std::fmt;
use std::rc::Rc;

use crate::source::ValueSource;

///
pub struct Single<S: ValueSource> {
    name: Cow<'static, str>,
    source: Rc<S>,
}

impl<S> fmt::Display for Single<S>
where
    S: ValueSource,
    S::Value: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.name)?;
        if let Some(v) = self.source.value() {
            v.fmt(f)
        } else {
            f.write_str("???")
        }
    }
}
