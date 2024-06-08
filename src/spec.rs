// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Status line specification helpers

use std::collections::hash_map::{self, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};

use crate::entry;
use crate::read;

/// Create entries based on command line arguments
///
/// Associated [read::Item]s will be appended to `items`.
pub fn entries(items: &mut Vec<read::Item>) -> Result<Vec<entry::Formatter>> {
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
    entries: &mut Vec<entry::Formatter>,
    installer: &mut ReadItemInstaller<'_>,
) -> Result<()> {
    use entry::Entry;

    match spec.main {
        "datetime" | "time" | "dt" | "t" => {
            spec.no_subs()?;
            entries.push(entry::LocalTime.into_fmt())
        }
        "load" | "l" => {
            spec.no_subs()?;
            let entry = installer
                .default::<read::Word<Option<f32>>>("/proc/loadavg", 64)?
                .with_precision(2)
                .with_label("load")
                .into_fmt();
            entries.push(entry);
        }
        "pressure" | "pres" | "psi" | "p" => {
            spec.parsed_subs_or([Ok(PSI::Cpu), Ok(PSI::Memory), Ok(PSI::Io)])
                .try_for_each(|i| {
                    let indicator = i?;
                    let entry = installer
                        .default::<read::PSI>(indicator.path(), 128)?
                        .with_precision(2)
                        .with_label(indicator)
                        .into_fmt();
                    entries.push(entry);
                    anyhow::Ok(())
                })?;
        }
        _ => anyhow::bail!("Unknown main spec: '{}'", spec.main),
    }
    Ok(())
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
    processors: HashMap<PathBuf, std::rc::Rc<dyn std::any::Any>>,
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
        path: impl Into<PathBuf>,
        buf_size: usize,
    ) -> Result<read::Ref<P>> {
        self.install(path, buf_size, Default::default())
    }

    /// Install a given [read::BufProcessor]
    pub fn install<P: read::BufProcessor + 'static>(
        &mut self,
        path: impl Into<PathBuf>,
        buf_size: usize,
        processor: P,
    ) -> Result<read::Ref<P>> {
        match self.processors.entry(path.into()) {
            hash_map::Entry::Occupied(entry) => {
                entry.get().clone().downcast().map_err(|_| {
                    anyhow::anyhow!("Existing processor for file has incompatible type")
                })
            }
            hash_map::Entry::Vacant(entry) => {
                let processor: read::Ref<P> = read::Ref::new(processor.into());
                let file = std::fs::File::open(entry.key())
                    .with_context(|| format!("Could not open {}", entry.key().display()))?;
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
