use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use regex::Regex;
use rustc_hash::FxHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    Case,
    Word,
    Regex,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Options {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

impl Options {
    #[must_use]
    pub fn holds(self, rule: Rule) -> bool {
        match rule {
            Rule::Case => self.case_sensitive,
            Rule::Word => self.whole_word,
            Rule::Regex => self.regex,
        }
    }

    #[must_use]
    pub fn toggled(mut self, rule: Rule) -> Self {
        let enabled = self.holds(rule);
        match rule {
            Rule::Case => self.case_sensitive = !enabled,
            Rule::Word => self.whole_word = !enabled,
            Rule::Regex => self.regex = !enabled,
        }
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct Query {
    pattern: Arc<str>,
    options: Options,
    expression: Option<Arc<str>>,
    matcher: Option<Arc<Regex>>,
    stamp: u64,
}

impl Query {
    #[must_use]
    pub fn new(pattern: &str, options: Options) -> Self {
        let stamp = stamp_of(pattern, options);
        if pattern.is_empty() {
            return Self {
                pattern: Arc::from(pattern),
                options,
                expression: None,
                matcher: None,
                stamp,
            };
        }
        let expression = expression(pattern, options);
        let matcher = Regex::new(&expression).ok().map(Arc::new);
        Self {
            pattern: Arc::from(pattern),
            options,
            expression: Some(Arc::from(expression.as_str())),
            matcher,
            stamp,
        }
    }

    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    #[must_use]
    pub fn options(&self) -> Options {
        self.options
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pattern.is_empty()
    }

    #[must_use]
    pub fn is_readable(&self) -> bool {
        self.matcher.is_some()
    }

    #[must_use]
    pub fn is_broken(&self) -> bool {
        !self.is_empty() && self.matcher.is_none()
    }

    #[must_use]
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    #[must_use]
    pub fn expression(&self) -> Option<&str> {
        self.expression.as_deref()
    }

    pub fn matches<'a>(&'a self, text: &'a str) -> impl Iterator<Item = Range<usize>> + 'a {
        self.matcher
            .as_deref()
            .into_iter()
            .flat_map(move |matcher| matcher.find_iter(text))
            .map(|found| found.range())
            .filter(|range| !range.is_empty())
    }

    pub fn find_all_into(&self, text: &str, into: &mut Vec<Range<usize>>) {
        into.clear();
        let Some(matcher) = &self.matcher else {
            return;
        };
        into.extend(
            matcher
                .find_iter(text)
                .map(|found| found.range())
                .filter(|range| !range.is_empty()),
        );
    }

    #[must_use]
    pub fn find_all(&self, text: &str) -> Vec<Range<usize>> {
        let mut found = Vec::new();
        self.find_all_into(text, &mut found);
        found
    }

    #[must_use]
    pub fn count(&self, text: &str) -> usize {
        let Some(matcher) = &self.matcher else {
            return 0;
        };
        matcher
            .find_iter(text)
            .filter(|found| !found.is_empty())
            .count()
    }

    #[must_use]
    pub fn expand_replacement(&self, matched: &str, replacement: &str) -> String {
        let Some(matcher) = self.matcher.as_ref().filter(|_| self.options.regex) else {
            return replacement.to_owned();
        };
        let Some(captures) = matcher.captures(matched) else {
            return replacement.to_owned();
        };
        let mut expanded = String::new();
        captures.expand(replacement, &mut expanded);
        expanded
    }
}

impl PartialEq for Query {
    fn eq(&self, other: &Self) -> bool {
        self.stamp == other.stamp
    }
}

impl Eq for Query {}

fn expression(pattern: &str, options: Options) -> String {
    let body = match options.regex {
        true => Cow::Borrowed(pattern),
        false => Cow::Owned(regex::escape(pattern)),
    };
    let case = match options.case_sensitive {
        true => "(?-i)",
        false => "(?i)",
    };
    match options.whole_word {
        true => format!(r"{case}\b(?:{body})\b"),
        false => format!("{case}(?:{body})"),
    }
}

fn stamp_of(pattern: &str, options: Options) -> u64 {
    let mut hasher = FxHasher::default();
    pattern.hash(&mut hasher);
    options.hash(&mut hasher);
    hasher.finish()
}
