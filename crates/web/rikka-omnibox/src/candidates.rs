use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::time::SystemTime;

use url::{Position, Url};

use crate::candidate::Candidate;
use crate::display::UrlDisplay;
use crate::error::{Error, Result};
use crate::fold::fold_into;
use crate::ranked::{MatchKind, Ranked};
use crate::ranking;
use crate::request::Request;

#[derive(Debug, Clone)]
pub struct Candidates<K> {
    pub(crate) entries: Vec<Entry<K>>,
    arena: String,
    pub(crate) max_frecency: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct Entry<K> {
    pub(crate) kind: MatchKind,
    pub(crate) key: Option<K>,
    pub(crate) url: Option<Url>,
    pub(crate) title: Box<str>,
    pub(crate) url_text: Range<u32>,
    pub(crate) title_text: Range<u32>,
    pub(crate) keyword_text: Range<u32>,
    pub(crate) host_start: u32,
    pub(crate) host_end: u32,
    pub(crate) identity: u64,
    pub(crate) visits: u32,
    pub(crate) typed: u32,
    pub(crate) last_visit: Option<SystemTime>,
    pub(crate) frecency: f32,
}

impl<K> Candidates<K> {
    #[must_use]
    pub fn new() -> Candidates<K> {
        Candidates {
            entries: Vec::new(),
            arena: String::new(),
            max_frecency: 0.0,
        }
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Candidates<K> {
        Candidates {
            entries: Vec::with_capacity(capacity),
            arena: String::with_capacity(capacity * 96),
            max_frecency: 0.0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.arena.clear();
        self.max_frecency = 0.0;
    }

    pub fn push(&mut self, candidate: Candidate<'_, K>) -> Result<()> {
        let url = if candidate.kind == MatchKind::Command {
            None
        } else {
            Some(
                Url::parse(candidate.url)
                    .map_err(|_| Error::InvalidUrl(candidate.url.to_owned()))?,
            )
        };
        let (url_text, host_start, host_end) = match &url {
            Some(url) => self.index_url(url),
            None => (self.empty_range(), 0, 0),
        };
        let title_text = self.index(|arena| fold_into(arena, candidate.title));
        let keyword_text = self.index(|arena| {
            for (index, keyword) in candidate.keywords.iter().enumerate() {
                if index > 0 {
                    arena.push(' ');
                }
                fold_into(arena, keyword);
            }
        });
        let frecency = candidate.frecency.max(0.0) as f32;
        self.max_frecency = self.max_frecency.max(frecency);
        self.entries.push(Entry {
            kind: candidate.kind,
            key: candidate.key,
            identity: url.as_ref().map_or(0, identity),
            url,
            title: candidate.title.into(),
            url_text,
            title_text,
            keyword_text,
            host_start,
            host_end,
            visits: candidate.visits,
            typed: candidate.typed,
            last_visit: candidate.last_visit,
            frecency,
        });
        Ok(())
    }

    #[must_use]
    pub fn rank(&self, request: &Request<'_>) -> Ranked<K>
    where
        K: Clone,
    {
        ranking::rank(self, request)
    }

    pub(crate) fn text(&self, range: &Range<u32>) -> &str {
        &self.arena[range.start as usize..range.end as usize]
    }

    fn empty_range(&self) -> Range<u32> {
        let at = offset(self.arena.len());
        at..at
    }

    fn index(&mut self, write: impl FnOnce(&mut String)) -> Range<u32> {
        let start = offset(self.arena.len());
        write(&mut self.arena);
        start..offset(self.arena.len())
    }

    fn index_url(&mut self, url: &Url) -> (Range<u32>, u32, u32) {
        let shown = UrlDisplay::matching(url);
        let text = shown.text();
        let host = shown.host().unwrap_or(0..0);
        let start = self.arena.len();
        fold_into(&mut self.arena, &text[..host.start]);
        let host_start = self.arena.len() - start;
        fold_into(&mut self.arena, &text[host.clone()]);
        let host_end = self.arena.len() - start;
        fold_into(&mut self.arena, &text[host.end..]);
        let www = if self.arena[start + host_start..start + host_end].starts_with("www.") {
            4
        } else {
            0
        };
        (
            offset(start)..offset(self.arena.len()),
            offset(host_start + www),
            offset(host_end),
        )
    }
}

impl<K> Default for Candidates<K> {
    fn default() -> Candidates<K> {
        Candidates::new()
    }
}

fn offset(position: usize) -> u32 {
    u32::try_from(position).expect("candidate text stays below 4 GiB")
}

pub(crate) fn identity(url: &Url) -> u64 {
    let mut hasher = DefaultHasher::new();
    if matches!(url.scheme(), "http" | "https") {
        let host = url.host_str().unwrap_or_default();
        host.strip_prefix("www.").unwrap_or(host).hash(&mut hasher);
        url.port().hash(&mut hasher);
        url.path().trim_end_matches('/').hash(&mut hasher);
        url.query().hash(&mut hasher);
    } else {
        url[..Position::AfterQuery].hash(&mut hasher);
    }
    hasher.finish().max(1)
}
