use std::time::SystemTime;

use crate::ranked::MatchKind;

#[derive(Debug, Clone)]
#[must_use]
pub struct Candidate<'a, K> {
    pub(crate) kind: MatchKind,
    pub(crate) key: Option<K>,
    pub(crate) url: &'a str,
    pub(crate) title: &'a str,
    pub(crate) keywords: Vec<&'a str>,
    pub(crate) visits: u32,
    pub(crate) typed: u32,
    pub(crate) last_visit: Option<SystemTime>,
    pub(crate) frecency: f64,
}

impl<'a, K> Candidate<'a, K> {
    fn new(kind: MatchKind, key: Option<K>, url: &'a str, title: &'a str) -> Self {
        Candidate {
            kind,
            key,
            url,
            title,
            keywords: Vec::new(),
            visits: 0,
            typed: 0,
            last_visit: None,
            frecency: 0.0,
        }
    }

    pub fn open_tab(key: K, url: &'a str, title: &'a str) -> Self {
        Candidate::new(MatchKind::OpenTab, Some(key), url, title)
    }

    pub fn pinned(key: K, url: &'a str, title: &'a str) -> Self {
        Candidate::new(MatchKind::Pinned, Some(key), url, title)
    }

    pub fn favorite(key: K, url: &'a str, title: &'a str) -> Self {
        Candidate::new(MatchKind::Favorite, Some(key), url, title)
    }

    pub fn archived(key: K, url: &'a str, title: &'a str) -> Self {
        Candidate::new(MatchKind::Archived, Some(key), url, title)
    }

    pub fn history(url: &'a str, title: &'a str) -> Self {
        Candidate::new(MatchKind::History, None, url, title)
    }

    pub fn command(key: K, name: &'a str) -> Self {
        Candidate::new(MatchKind::Command, Some(key), "", name)
    }

    pub fn visits(mut self, visits: u32) -> Self {
        self.visits = visits;
        self
    }

    pub fn typed(mut self, typed: u32) -> Self {
        self.typed = typed;
        self
    }

    pub fn last_visit(mut self, at: SystemTime) -> Self {
        self.last_visit = Some(at);
        self
    }

    pub fn frecency(mut self, frecency: f64) -> Self {
        self.frecency = frecency;
        self
    }

    pub fn keywords(mut self, keywords: impl IntoIterator<Item = &'a str>) -> Self {
        self.keywords.extend(keywords);
        self
    }
}
