// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Status line specification helpers

use std::collections::hash_map::{self, HashMap};
use std::fmt;
use std::fs::File;
use std::os::linux::fs::MetadataExt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::entry::{self, Entry};
use crate::meminfo;
use crate::power;
use crate::read;
use crate::scale;
use crate::source::{self, LowerRate};

/// Base interval for updates
const BASE_INTERVAL: Duration = Duration::from_secs(5);

/// Create entries based on command line arguments
///
/// Associated [read::Item]s will be appended to `items`.
pub fn entries(items: &mut Vec<read::Item>) -> Result<Vec<Box<dyn fmt::Display>>> {
    let mut res = Default::default();
    let mut items = ReadItemInstaller::new(items);
    std::env::args().skip(1).try_for_each(|a| {
        apply(a.as_str().into(), &mut res, &mut items)
            .with_context(|| format!("Could not add entries for '{a}'"))
    })?;

    Ok(res)
}

/// Apply a given specification
///
/// Create entris and install [read::Item]s for a single given [Spec].
fn apply(
    spec: Spec<'_>,
    entries: &mut Vec<Box<dyn fmt::Display>>,
    installer: &mut ReadItemInstaller<'_>,
) -> Result<()> {
    match spec.main {
        "datetime" | "time" | "dt" | "t" => {
            spec.no_subs()?;
            entries.push(entry::LocalTime.into_fmt());
            Ok(())
        }
        "load" | "l" => apply_load(spec, entries, installer),
        "pressure" | "pres" | "psi" | "p" => apply_psi(spec, entries, installer),
        "memory" | "mem" | "m" => apply_meminfo(spec, entries, installer),
        "battery" | "bat" | "b" => apply_battery(spec, entries, installer),
        _ => anyhow::bail!("Unknown main spec: '{}'", spec.main),
    }
}

/// Aplly a load [Spec]
fn apply_load(
    spec: Spec<'_>,
    entries: &mut Vec<Box<dyn fmt::Display>>,
    installer: &mut ReadItemInstaller<'_>,
) -> Result<()> {
    spec.no_subs()?;
    let read = read::Simple::new(
        LowerRate::<f32>::new(BASE_INTERVAL),
        u8::is_ascii_whitespace,
    );
    let entry = installer
        .install("/proc/loadavg", 64, read)?
        .with_precision(2)
        .into_fmt();
    entries.push(entry::label("load"));
    entries.push(entry);
    Ok(())
}

/// Aplly a pressure [Spec]
fn apply_psi(
    spec: Spec<'_>,
    entries: &mut Vec<Box<dyn fmt::Display>>,
    installer: &mut ReadItemInstaller<'_>,
) -> Result<()> {
    spec.parsed_subs_or([Ok(PSI::Cpu), Ok(PSI::Memory), Ok(PSI::Io)])
        .try_for_each(|i| {
            let indicator = i?;
            let read: read::PSI<_> = LowerRate::new(BASE_INTERVAL).into();
            let entry = installer
                .install(indicator.path(), 128, read)?
                .with_precision(2)
                .into_fmt();
            entries.push(entry::label(indicator));
            entries.push(entry);
            Ok(())
        })
}

/// Aplly a meminfo [Spec]
fn apply_meminfo(
    spec: Spec<'_>,
    entries: &mut Vec<Box<dyn fmt::Display>>,
    installer: &mut ReadItemInstaller<'_>,
) -> Result<()> {
    let source = installer.default::<meminfo::MemInfo>("/proc/meminfo", 1536)?;
    spec.parsed_subs_or([Ok(meminfo::Item::Avail), Ok(meminfo::Item::Free)])
        .try_for_each(|i| {
            let item = i?;
            let entry = entry::mapped(source.clone(), move |i| i[item].map(|i| i as f64))
                .autoscaled(1.5, scale::BinScale::Kibi)
                .with_precision(1)
                .with_unit('B')
                .into_fmt();
            entries.push(entry::label(item));
            entries.push(entry);
            Ok(())
        })
}

/// Aplly a battery [Spec]
fn apply_battery(
    spec: Spec<'_>,
    entries: &mut Vec<Box<dyn fmt::Display>>,
    installer: &mut ReadItemInstaller<'_>,
) -> Result<()> {
    use power::Status;
    use read::Simple;
    use source::{MovingAverage, Source};

    spec.parsed_subs_or(power::supplies()?)
        .filter_map(Result::ok)
        .filter(|p| p.kind().ok() == Some(power::Kind::Battery))
        .try_for_each(|p| {
            let full = Simple::new(
                LowerRate::new(Duration::from_secs(120)),
                u8::is_ascii_whitespace,
            );
            let full = installer.install_file(p.charge_full_file()?, 16, full)?;
            let now = Simple::new(
                LowerRate::new(Duration::from_secs(15)),
                u8::is_ascii_whitespace,
            );
            let now = installer.install_file(p.charge_now_file()?, 16, now)?;
            let soc = entry::zipped(full, now.clone(), |f: &f32, n: &f32| Some(100. * n / f))
                .with_precision(0)
                .with_unit('%')
                .into_fmt();

            let status = installer.install_file(
                p.status_file()?,
                16,
                Simple::new(LowerRate::new(BASE_INTERVAL), u8::is_ascii_control),
            )?;
            let avg = MovingAverage::<f32>::new(Duration::from_secs(60)).gated_with({
                let status = status.clone();
                move || status.borrow().value() == Some(Status::Discharging)
            });
            let current = installer.install_file(
                p.current_now_file()?,
                16,
                Simple::new(avg, u8::is_ascii_whitespace),
            )?;
            let status = move || {
                let status = status.borrow().value()?;
                let display = (status == Status::Discharging)
                    .then(|| {
                        let charge = now.borrow().value();
                        let current = current.borrow().value().filter(|c| c.is_normal());
                        Option::zip(current, charge)
                    })
                    .flatten()
                    .map(|(i, c)| c * 3600. / i) // µAh * s/h / µA
                    .autoscaled(1.5, scale::Duration::Second)
                    .with_precision(1)
                    .display()
                    .map_or(either::Either::Left(status.symbol()), either::Either::Right);
                Some(display)
            };

            entries.push(entry::label(p.name().to_owned()));
            entries.push(soc);
            entries.push(status.into_fmt());
            Ok(())
        })
}

/// A single specification for status line entries
#[derive(PartialEq, Debug)]
struct Spec<'a> {
    pub main: &'a str,
    pub subs: Vec<&'a str>,
}

impl Spec<'_> {
    /// Ensure this spec has no sub specifications
    pub fn no_subs(&self) -> Result<&Self> {
        self.subs
            .is_empty()
            .then_some(self)
            .ok_or(anyhow::anyhow!("Spec has subspec when it shouldn't"))
    }

    /// Parse sub specifications into some type or use some default
    pub fn parsed_subs_or<'a, I, T>(
        &'a self,
        default: I,
    ) -> impl Iterator<Item = Result<T, T::Err>> + 'a
    where
        I: IntoIterator<Item = Result<T, T::Err>>,
        I::IntoIter: 'a,
        T: FromStr,
    {
        if self.subs.is_empty() {
            either::Either::Left(default.into_iter())
        } else {
            either::Either::Right(self.parsed_subs())
        }
    }

    /// Parse sub specifications into some type
    pub fn parsed_subs<T: FromStr>(&self) -> impl Iterator<Item = Result<T, T::Err>> + '_ {
        self.subs.iter().map(|s| s.parse())
    }
}

impl<'a> From<&'a str> for Spec<'a> {
    fn from(s: &'a str) -> Self {
        let (main, subs) = s.split_once(':').unwrap_or((s, Default::default()));
        let subs = subs.split(',').filter(|s| !s.is_empty()).collect();
        Self { main, subs }
    }
}

/// PSI-specific sub specification
#[derive(Copy, Clone, Debug)]
enum PSI {
    Cpu,
    Memory,
    Io,
}

impl PSI {
    /// Get the path for the file from which to pull this info
    pub fn path(self) -> &'static Path {
        let path = match self {
            Self::Cpu => "/proc/pressure/cpu",
            Self::Memory => "/proc/pressure/memory",
            Self::Io => "/proc/pressure/io",
        };
        Path::new(path)
    }
}

impl FromStr for PSI {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cpu" | "c" => Ok(Self::Cpu),
            "memory" | "mem" | "m" => Ok(Self::Memory),
            "io" => Ok(Self::Io),
            _ => Err(anyhow::anyhow!("Not a valid sub spec for PSI: {s}")),
        }
    }
}

impl fmt::Display for PSI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cpu => "cpu",
            Self::Memory => "mem",
            Self::Io => "io",
        })
    }
}

/// Installer for [read::Item]s, making sure we only have one per path
struct ReadItemInstaller<'i> {
    items: &'i mut Vec<read::Item>,
    processors: HashMap<(u64, u64), std::rc::Rc<dyn std::any::Any>>,
}

impl<'i> ReadItemInstaller<'i> {
    /// Create a new installer pusing [read::Item]s to the given [Vec]
    pub fn new(items: &'i mut Vec<read::Item>) -> Self {
        Self {
            items,
            processors: Default::default(),
        }
    }

    /// Install a [read::BufProcessor]'s [Default] value
    pub fn default<P: read::BufProcessor + Default + 'static>(
        &mut self,
        path: impl AsRef<Path>,
        buf_size: usize,
    ) -> Result<read::Ref<P>> {
        self.install(path, buf_size, Default::default())
    }

    /// Install a [read::BufProcessor]'s [Default] value
    pub fn default_file<P: read::BufProcessor + Default + 'static>(
        &mut self,
        file: File,
        buf_size: usize,
    ) -> Result<read::Ref<P>> {
        self.install_file(file, buf_size, Default::default())
    }

    /// Install a given [read::BufProcessor]
    pub fn install<P: read::BufProcessor + 'static>(
        &mut self,
        path: impl AsRef<Path>,
        buf_size: usize,
        processor: P,
    ) -> Result<read::Ref<P>> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("Could not open {}", path.display()))?;
        self.install_file(file, buf_size, processor)
    }

    /// Install a given [read::BufProcessor]
    pub fn install_file<P: read::BufProcessor + 'static>(
        &mut self,
        file: File,
        buf_size: usize,
        processor: P,
    ) -> Result<read::Ref<P>> {
        let metadata = file
            .metadata()
            .context("Could not retrieve metadata for file")?;
        let key = (metadata.st_dev(), metadata.st_ino());
        match self.processors.entry(key) {
            hash_map::Entry::Occupied(entry) => {
                entry.get().clone().downcast().map_err(|_| {
                    anyhow::anyhow!("Existing processor for file has incompatible type")
                })
            }
            hash_map::Entry::Vacant(entry) => {
                let processor: read::Ref<P> = read::Ref::new(processor.into());
                self.items
                    .push(read::Item::new(file, buf_size, processor.clone()));
                entry.insert(processor.clone());
                Ok(processor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_parse_simple() {
        assert_eq!(
            Spec::from("foo"),
            Spec {
                main: "foo".into(),
                subs: Default::default(),
            },
        )
    }

    #[test]
    fn spec_parse_with_subs() {
        assert_eq!(
            Spec::from("foo:bar,baz"),
            Spec {
                main: "foo".into(),
                subs: vec!["bar".into(), "baz".into()],
            },
        )
    }

    #[test]
    fn spec_no_subs_1() {
        Spec {
            main: Default::default(),
            subs: Default::default(),
        }
        .no_subs()
        .expect("No subs failed");
    }

    #[test]
    fn spec_no_subs_2() {
        assert!(Spec {
            main: Default::default(),
            subs: vec!["boo".into()]
        }
        .no_subs()
        .is_err())
    }

    #[test]
    fn spec_parsed_subs() {
        let subs: Vec<Option<u32>> = Spec {
            main: Default::default(),
            subs: vec!["1".into(), "2".into(), "three".into()],
        }
        .parsed_subs()
        .map(Result::ok)
        .collect();
        assert_eq!(subs, [Some(1), Some(2), None])
    }

    #[test]
    fn spec_parsed_subs_or_1() {
        let subs: Vec<Option<u32>> = Spec {
            main: Default::default(),
            subs: vec!["1".into(), "2".into(), "three".into()],
        }
        .parsed_subs_or([Ok(4), Ok(5), Ok(6)])
        .map(Result::ok)
        .collect();
        assert_eq!(subs, [Some(1), Some(2), None])
    }

    #[test]
    fn spec_parsed_subs_or_2() {
        let subs: Vec<Option<u32>> = Spec {
            main: Default::default(),
            subs: Default::default(),
        }
        .parsed_subs_or([Ok(4), Ok(5), Ok(6)])
        .map(Result::ok)
        .collect();
        assert_eq!(subs, [Some(4), Some(5), Some(6)])
    }
}
