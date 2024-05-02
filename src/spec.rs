// SPDX-License-Identifier: MIT
// Copyright Julian Ganz 2024
//! Status line specification helpers

use std::str::FromStr;

use anyhow::Result;

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
