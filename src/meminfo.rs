// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Utilities for handling `/proc/meminfo`

/// Values found in `/proc/meminfo`
pub type MemInfo = enum_map::EnumMap<Item, Option<u64>>;

/// Identification of a [MemInfo] item
#[derive(Copy, Clone, enum_map::Enum)]
pub enum Item {
    Total,
    Free,
    Avail,
    SwapTotal,
    SwapFree,
}
