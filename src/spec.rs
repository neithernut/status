// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Status line specification helpers

use std::collections::hash_map::{self, HashMap};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};

use crate::read;

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
