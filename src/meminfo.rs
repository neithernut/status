// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Utilities for handling `/proc/meminfo`

use std::fmt;

use crate::read::BufProcessor;
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

impl BufProcessor for MemInfo {
    fn process(&mut self, buf: &[u8]) {
        *self = buf
            .split(|c| *c == b'\n')
            .map(std::str::from_utf8)
            .filter_map(Result::ok)
            .filter_map(|l| l.split_once(':'))
            .filter_map(|(k, v)| {
                let key = match k {
                    "MemTotal" => Item::Total,
                    "MemFree" => Item::Free,
                    "MemAvailable" => Item::Avail,
                    "SwapTotal" => Item::SwapTotal,
                    "SwapFree" => Item::SwapFree,
                    _ => return None,
                };
                let value = v
                    .strip_suffix("kB")
                    .map(str::trim)
                    .and_then(|v| v.parse().ok());
                Some((key, value))
            })
            .collect();
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

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Total => "tot",
            Self::Free => "free",
            Self::Avail => "avail",
            Self::SwapTotal => "totswap",
            Self::SwapFree => "free swap",
        };
        f.write_str(name)
    }
}

impl std::str::FromStr for Item {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "total" | "tot" | "t" => Ok(Self::Total),
            "free" | "f" => Ok(Self::Free),
            "available" | "availible" | "avail" | "a" => Ok(Self::Avail),
            "totalswap" | "totsw" | "ts" => Ok(Self::SwapTotal),
            "freeswap" | "freesw" | "fs" => Ok(Self::SwapFree),
            _ => Err(anyhow::anyhow!("Not a valid sub spec for PSI: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let buf = concat!(
            "MemTotal:       32166268 kB\n",
            "MemFree:        27646592 kB\n",
            "MemAvailable:   30632568 kB\n",
            "Buffers:          432404 kB\n",
            "Cached:          2595616 kB\n",
            "Mlocked:           11808 kB\n",
            "SwapTotal:          1024 kB\n",
            "SwapFree:              0 kB\n",
            "Dirty:                36 kB\n",
            "Writeback:             0 kB\n",
            "AnonPages:        459056 kB\n",
            "Mapped:           376292 kB\n",
            "Shmem:             96444 kB\n",
        )
        .as_bytes();
        let mut meminfo = MemInfo::default();
        meminfo.process(buf);
        assert_eq!(meminfo[Item::Total], Some(32166268));
        assert_eq!(meminfo[Item::Free], Some(27646592));
        assert_eq!(meminfo[Item::Avail], Some(30632568));
        assert_eq!(meminfo[Item::SwapTotal], Some(1024));
        assert_eq!(meminfo[Item::SwapFree], Some(0));
    }

    #[test]
    fn incomplete() {
        let buf = concat!(
            "MemTotal:       32166268 kB\n",
            "MemFree:        27646592 kB\n",
            "MemAvailable:   30632568 kB\n",
            "Buffers:          432404 kB\n",
        )
        .as_bytes();
        let mut meminfo = MemInfo::default();
        meminfo[Item::SwapTotal] = Some(0);
        meminfo[Item::SwapFree] = Some(0);
        meminfo.process(buf);
        assert_eq!(meminfo[Item::Total], Some(32166268));
        assert_eq!(meminfo[Item::Free], Some(27646592));
        assert_eq!(meminfo[Item::Avail], Some(30632568));
        assert_eq!(meminfo[Item::SwapTotal], None);
        assert_eq!(meminfo[Item::SwapFree], None);
    }
}
