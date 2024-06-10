// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Uitilities related to entities in `/sys/class/power_supply`

use std::path;

use anyhow::{Context, Result};

/// Representation of a power supply interface in `/sys/class/power_supply/`
pub struct Supply {
    name: String,
    dir: openat::Dir,
}

impl Supply {
    /// Get the name of this power supply
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    fn new(name: String, path: impl AsRef<path::Path>) -> Result<Self> {
        let path = path.as_ref();
        openat::Dir::open(path)
            .map(|dir| Self { name, dir })
            .with_context(|| format!("Could not open dir {}", path.display()))
    }
}

impl std::str::FromStr for Supply {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut path = path::PathBuf::from("/sys/class/power_supply/");
        path.push(s);
        Self::new(s.into(), path)
    }
}
