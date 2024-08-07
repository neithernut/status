// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Uitilities related to entities in `/sys/class/power_supply`

use std::fs::File;
use std::io::Read;
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

    /// Get the kind of power supply
    pub fn kind(&self) -> Result<Kind> {
        let mut buf = String::new();
        self.open_file("type")?
            .read_to_string(&mut buf)
            .context("Could not determining type")?;
        buf.trim().parse().context("Could not parse type")
    }

    /// Open the `charge_now` file for this source
    ///
    /// The file contains the current charge, in µAh.
    pub fn charge_now_file(&self) -> Result<File> {
        self.open_file("charge_now")
    }

    /// Open the `charge_full` file for this source
    ///
    /// The file contains the charge when the battery is full, in µAh.
    pub fn charge_full_file(&self) -> Result<File> {
        self.open_file("charge_full")
    }

    /// Open the `charge_empty` file for this source
    ///
    /// The file contains the charge when the battery is empty, in µAh.
    pub fn charge_empty_file(&self) -> Result<File> {
        self.open_file("charge_empty")
    }

    /// Open the `current_now` file for this source
    ///
    /// The file contains the charge when the battery is empty, in µA.
    pub fn current_now_file(&self) -> Result<File> {
        self.open_file("current_now")
    }

    /// Open the `status` file for this source
    pub fn status_file(&self) -> Result<File> {
        self.open_file("status")
    }

    /// Open a specific file
    fn open_file(&self, name: &str) -> Result<File> {
        self.dir
            .open_file(name)
            .with_context(|| format!("Could not open '{name}'"))
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

/// Kind of power supply
#[derive(Copy, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum Kind {
    Battery,
    UPS,
    Mains,
    USB,
    Wireless,
}

impl std::str::FromStr for Kind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Battery" => Ok(Self::Battery),
            "UPS" => Ok(Self::UPS),
            "Mains" => Ok(Self::Mains),
            "USB" => Ok(Self::USB),
            "Wireless" => Ok(Self::Wireless),
            s => Err(anyhow::anyhow!("Invalid kind '{s}'")),
        }
    }
}

/// Power supply status
#[derive(Copy, Clone, PartialEq)]
pub enum Status {
    Unknown,
    Charging,
    Discharging,
    NotCharging,
    Full,
}

impl Status {
    /// Get a symbol corresponding to the battery status
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Unknown => "❓️",
            Self::Charging => "⚡️",
            Self::Discharging => "🔋",
            Self::NotCharging => "⭐️",
            Self::Full => "✨️",
        }
    }
}

impl std::str::FromStr for Status {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Unknown" => Ok(Self::Unknown),
            "Charging" => Ok(Self::Charging),
            "Discharging" => Ok(Self::Discharging),
            "Not charging" => Ok(Self::NotCharging),
            "Full" => Ok(Self::Full),
            s => Err(anyhow::anyhow!("Unknown status '{s}'")),
        }
    }
}

/// Get all power supplies
pub fn supplies() -> Result<impl Iterator<Item = Result<Supply>>> {
    let list = std::fs::read_dir("/sys/class/power_supply/")
        .context("Could not access /sys/class/power_supply/")?
        .map(|e| {
            let entry = e.context("Could not read entry")?;
            let name = entry
                .file_name()
                .into_string()
                .unwrap_or_else(|n| n.to_string_lossy().into());
            let path = entry.path();
            Supply::new(name, path)
        });
    Ok(list)
}
