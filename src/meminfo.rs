// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Utilities for handling `/proc/meminfo`

use crate::source::Source;

/// Values found in `/proc/meminfo`
pub type MemInfo = enum_map::EnumMap<Item, Option<u64>>;

impl Source for MemInfo {
    type Value = Self;

    type Borrow<'a> = &'a Self::Value;

    fn value(&self) -> Option<Self::Borrow<'_>> {
        Some(self)
    }
}

/// Identification of a [MemInfo] item
#[derive(Copy, Clone, enum_map::Enum)]
pub enum Item {
    Total,
    Free,
    Avail,
    SwapTotal,
    SwapFree,
}
