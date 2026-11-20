use std::collections::HashMap;
use url::Url;

use crate::action::Action;
use crate::candidates::{Candidates, Entry, identity};
use crate::classify::{Destination, Intent};
use crate::display::UrlDisplay;
use crate::fold::folded;
use crate::matching::{self, Fields, Terms};
use crate::ranked::{Match, MatchKind, Ranked};
use crate::request::{Request, Surface};
use crate::search::Search;

const SHORTLIST: usize = 64;
const INLINE_BASE: u32 = 1410;
const INLINE_SPREAD: u32 = 89;
const TYPED_URL: u32 = 1200;
const TYPED_URL_VISITED: u32 = 1400;
const TYPED_SEARCH: u32 = 1300;
const SEARCH_FOR_URL: u32 = 900;
const ALTERNATIVE: u32 = 1100;
const ALTERNATIVE_VISITED: u32 = 1350;
const SUGGESTION: u32 = 1099;
const SUGGESTED_URL: u32 = 1050;
const MATCH_BASE: f32 = 550.0;
const MATCH_QUALITY: f32 = 450.0;
const MATCH_POPULARITY: f32 = 250.0;
const MATCH_COVERAGE: f32 = 50.0;
const MATCH_CEILING: f32 = 1399.0;
const COMMAND_BASE: f32 = 650.0;
const COMMAND_QUALITY: f32 = 600.0;
const COMMAND_EXACT: u32 = 1350;
const EMPTY_BASE: f32 = 300.0;
const EMPTY_POPULARITY: f32 = 300.0;
const DAY: f32 = 86_400.0;
const STRONG_VISITS: u32 = 3;
const STRONG_RECENCY: f32 = 0.2;

#[derive(Debug, Clone, Copy)]
struct Hit {
    index: usize,
    score: u32,
}

#[derive(Debug, Clone, Copy)]
struct Inline {
    index: usize,
    fill_start: usize,
    host_only: bool,
    score: u32,
}

#[derive(Debug)]
struct Scanned {
    hits: Vec<Hit>,
    inline: Option<Inline>,
    typed_entry: Option<usize>,
    alternative_visited: bool,
}

#[derive(Debug)]
enum Source<'r> {
    Entry(usize),
    Typed(&'r Destination, Option<usize>),
    TypedSearch(Search),
    Suggestion(usize),
    Inline(usize, Url),
}

#[derive(Debug)]
struct Row<'r> {
    kind: MatchKind,
    score: u32,
    source: Source<'r>,
}

pub(crate) fn rank<K: Clone>(candidates: &Candidates<K>, request: &Request<'_>) -> Ranked<K> {
    let typed = match request.input.search() {
        Some(search) if search.by_keyword() => search.terms(),
        _ => request.input.text(),
    };
    let normalized = folded(typed);
    let terms = Terms::new(&normalized);
    let url_input = terms.single().unwrap_or_default();
    Scan {
        candidates,
        request,
        terms: &terms,
        normalized: &normalized,
        url_input,
    }
    .run()
}

struct Scan<'r, 't, K> {
    candidates: &'r Candidates<K>,
    request: &'r Request<'r>,
    terms: &'t Terms<'t>,
    normalized: &'t str,
    url_input: &'t str,
}

impl<'r, 't, K: Clone> Scan<'r, 't, K> {
    fn run(self) -> Ranked<K> {
        let scanned = self.scan();
        let inline = scanned
            .inline
            .and_then(|inline| self.inline_suffix(inline).map(|suffix| (inline, suffix)));
        let mut rows = Vec::with_capacity(scanned.hits.len() + self.request.suggestions.len() + 3);
        let mut navigation: HashMap<u64, usize> = HashMap::new();
        if let Some((inline_match, _)) = &inline {
            let target = self.inline_target(*inline_match);
            navigation.insert(identity(&target), rows.len());
            rows.push(Row {
                kind: MatchKind::History,
                score: inline_match.score,
                source: Source::Inline(inline_match.index, target),
            });
        }
        self.push_typed(&mut rows, &mut navigation, &scanned);
        self.push_entries(&mut rows, &mut navigation, &scanned.hits);
        self.push_suggestions(&mut rows, &navigation);

        let ordered = self.order(rows, inline.is_some());
        Ranked {
            matches: ordered.into_iter().map(|row| self.to_match(row)).collect(),
            inline: inline.map(|(_, suffix)| suffix),
        }
    }

    fn scan(&self) -> Scanned {
        let input = self.request.input;
        let typed_identity = input
            .destination()
            .map(|destination| identity(destination.url()));
        let alternative_host = input
            .alternative()
            .and_then(|destination| destination.url().host_str());
        let inline_allowed = self.request.surface == Surface::AddressField
            && !self.request.just_deleted
            && !input.ends_with_space()
            && !self.url_input.is_empty()
            && input.search().is_none_or(|search| !search.by_keyword());
        let mut scanned = Scanned {
            hits: Vec::new(),
            inline: None,
            typed_entry: None,
            alternative_visited: false,
        };

        for (index, entry) in self.candidates.entries.iter().enumerate() {
            let fields = self.fields(entry);
            if typed_identity == Some(entry.identity)
                && scanned
                    .typed_entry
                    .is_none_or(|known| entry.typed > self.candidates.entries[known].typed)
            {
                scanned.typed_entry = Some(index);
            }
            if let Some(host) = alternative_host {
                let shown = &fields.url[..fields.host_end];
                scanned.alternative_visited |=
                    shown == host || shown.strip_prefix("www.") == Some(host);
            }
            if self.terms.is_empty() {
                if input.is_empty() && entry.kind != MatchKind::Command {
                    let score =
                        EMPTY_BASE + EMPTY_POPULARITY * self.popularity(entry) + bonus(entry.kind);
                    scanned.hits.push(Hit {
                        index,
                        score: score as u32,
                    });
                }
                continue;
            }
            let quality = self.terms.quality(fields);
            if quality <= 0.0 {
                continue;
            }
            let score = if entry.kind == MatchKind::Command {
                command_score(quality, self.terms, self.normalized, fields.title)
            } else {
                self.entry_score(entry, fields, quality)
            };
            scanned.hits.push(Hit { index, score });
            if inline_allowed
                && let Some(candidate) = self.inline_candidate(index, entry, fields)
                && scanned
                    .inline
                    .is_none_or(|best| candidate.score > best.score)
            {
                scanned.inline = Some(candidate);
            }
        }

        let hits = &mut scanned.hits;
        if hits.len() > SHORTLIST {
            hits.select_nth_unstable_by(SHORTLIST, |a, b| b.score.cmp(&a.score));
            hits.truncate(SHORTLIST);
        }
        hits.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| {
                let kind = |hit: &Hit| self.candidates.entries[hit.index].kind.rank();
                kind(a).cmp(&kind(b))
            })
        });
        scanned
    }

    fn fields(&self, entry: &'r Entry<K>) -> Fields<'r> {
        Fields {
            url: self.candidates.text(&entry.url_text),
            host_start: entry.host_start as usize,
            host_end: entry.host_end as usize,
            title: self.candidates.text(&entry.title_text),
            keywords: self.candidates.text(&entry.keyword_text),
        }
    }

    fn popularity(&self, entry: &Entry<K>) -> f32 {
        let typed = entry.typed.min(50) as f32;
        let visits = entry.visits.min(500) as f32;
        let count = 1.0 - (-(typed * 3.0 + visits) / 12.0).exp();
        let recency = self.recency(entry);
        if self.candidates.max_frecency > 0.0 {
            let frecency = entry.frecency / self.candidates.max_frecency;
            0.4 * count + 0.3 * recency + 0.3 * frecency
        } else {
            0.6 * count + 0.4 * recency
        }
    }

    fn recency(&self, entry: &Entry<K>) -> f32 {
        entry.last_visit.map_or(0.0, |at| {
            let age = self
                .request
                .now
                .duration_since(at)
                .map_or(0.0, |age| age.as_secs_f32() / DAY);
            1.0 / (1.0 + age / 7.0)
        })
    }

    fn entry_score(&self, entry: &Entry<K>, fields: Fields<'_>, quality: f32) -> u32 {
        let mut score =
            MATCH_BASE + MATCH_QUALITY * quality + MATCH_POPULARITY * self.popularity(entry);
        if let Some(start) = self.prefix_start(fields) {
            let filled = fields
                .host_end
                .saturating_sub(start)
                .max(self.url_input.len());
            score += MATCH_COVERAGE * self.url_input.len() as f32 / filled.max(1) as f32;
        }
        (score + bonus(entry.kind)).min(MATCH_CEILING) as u32
    }

    fn prefix_start(&self, fields: Fields<'_>) -> Option<usize> {
        if self.url_input.is_empty() || fields.host_end == 0 {
            return None;
        }
        if fields.url.starts_with(self.url_input) {
            return Some(0);
        }
        let start = fields.host_start;
        (start > 0 && fields.url[start..].starts_with(self.url_input)).then_some(start)
    }

    fn inline_candidate(
        &self,
        index: usize,
        entry: &Entry<K>,
        fields: Fields<'_>,
    ) -> Option<Inline> {
        if !matches!(
            entry.kind,
            MatchKind::History | MatchKind::OpenTab | MatchKind::Pinned | MatchKind::Favorite
        ) {
            return None;
        }
        let fill_start = self.prefix_start(fields)?;
        let pinned = matches!(entry.kind, MatchKind::Pinned | MatchKind::Favorite);
        let strong = entry.typed > 0
            || pinned
            || (entry.visits >= STRONG_VISITS && self.recency(entry) >= STRONG_RECENCY);
        if !strong {
            return None;
        }
        let filled = fill_start + self.url_input.len();
        if filled == fields.host_end {
            return None;
        }
        let host_only = filled < fields.host_end;
        let bonus = entry.typed.saturating_mul(8) + entry.visits / 4 + if pinned { 10 } else { 0 };
        Some(Inline {
            index,
            fill_start,
            host_only,
            score: INLINE_BASE + bonus.min(INLINE_SPREAD),
        })
    }

    fn inline_target(&self, inline: Inline) -> Url {
        let url = self.candidates.entries[inline.index]
            .url
            .clone()
            .expect("inline candidates have URLs");
        if !inline.host_only {
            return url;
        }
        let mut root = url;
        root.set_path("/");
        root.set_query(None);
        root.set_fragment(None);
        root
    }

    fn inline_suffix(&self, inline: Inline) -> Option<String> {
        let entry = &self.candidates.entries[inline.index];
        let fields = self.fields(entry);
        let original = UrlDisplay::matching(entry.url.as_ref()?).into_text();
        let skip = fields.url[..inline.fill_start].chars().count() + self.url_input.chars().count();
        let end = if inline.host_only {
            fields.url[..fields.host_end].chars().count()
        } else {
            fields.url.chars().count()
        };
        let suffix: String = original.chars().take(end).skip(skip).collect();
        (!suffix.is_empty()).then_some(suffix)
    }

    fn push_typed(
        &self,
        rows: &mut Vec<Row<'r>>,
        navigation: &mut HashMap<u64, usize>,
        scanned: &Scanned,
    ) {
        let input = self.request.input;
        let visited = scanned.typed_entry.filter(|&index| {
            let entry = &self.candidates.entries[index];
            entry.typed > 0 || entry.visits > 0
        });
        match input.intent() {
            Intent::Empty => {}
            Intent::Navigate(destination) => {
                let score = if visited.is_some_and(|index| self.candidates.entries[index].typed > 0)
                {
                    TYPED_URL_VISITED
                } else {
                    TYPED_URL
                };
                navigation.insert(identity(destination.url()), rows.len());
                rows.push(Row {
                    kind: MatchKind::TypedUrl,
                    score,
                    source: Source::Typed(destination, visited),
                });
                rows.push(Row {
                    kind: MatchKind::TypedSearch,
                    score: SEARCH_FOR_URL,
                    source: Source::TypedSearch(
                        self.request.engines.default_engine().search(input.text()),
                    ),
                });
            }
            Intent::Search(search) => {
                rows.push(Row {
                    kind: MatchKind::TypedSearch,
                    score: TYPED_SEARCH,
                    source: Source::TypedSearch(search.clone()),
                });
                if let Some(alternative) = input.alternative() {
                    navigation.insert(identity(alternative.url()), rows.len());
                    rows.push(Row {
                        kind: MatchKind::TypedUrl,
                        score: if scanned.alternative_visited {
                            ALTERNATIVE_VISITED
                        } else {
                            ALTERNATIVE
                        },
                        source: Source::Typed(alternative, visited),
                    });
                }
            }
        }
    }

    fn push_entries(
        &self,
        rows: &mut Vec<Row<'r>>,
        navigation: &mut HashMap<u64, usize>,
        hits: &[Hit],
    ) {
        let mut merged: HashMap<u64, usize> = HashMap::with_capacity(hits.len());
        for hit in hits {
            let entry = &self.candidates.entries[hit.index];
            if entry.identity != 0
                && !entry.kind.is_tab()
                && let Some(&row) = navigation.get(&entry.identity)
            {
                if let Source::Typed(destination, None) = rows[row].source {
                    rows[row].source = Source::Typed(destination, Some(hit.index));
                }
                continue;
            }
            if entry.identity != 0 {
                if let Some(&row) = merged.get(&entry.identity) {
                    let existing = &mut rows[row];
                    existing.score = existing.score.max(hit.score);
                    if entry.kind.rank() < existing.kind.rank() {
                        existing.kind = entry.kind;
                        existing.source = Source::Entry(hit.index);
                    }
                    continue;
                }
                merged.insert(entry.identity, rows.len());
            }
            rows.push(Row {
                kind: entry.kind,
                score: hit.score,
                source: Source::Entry(hit.index),
            });
        }
        for (identity, row) in merged {
            navigation.entry(identity).or_insert(row);
        }
    }

    fn push_suggestions(&self, rows: &mut Vec<Row<'r>>, navigation: &HashMap<u64, usize>) {
        let typed = folded(
            self.request
                .input
                .search()
                .map_or(self.request.input.text(), Search::terms),
        );
        let mut seen: Vec<String> = Vec::with_capacity(self.request.suggestions.len());
        for (index, suggestion) in self.request.suggestions.iter().enumerate() {
            let order = u32::try_from(index).unwrap_or(u32::MAX);
            match &suggestion.url {
                Some(url) => {
                    if navigation.contains_key(&identity(url)) {
                        continue;
                    }
                    rows.push(Row {
                        kind: MatchKind::SuggestedUrl,
                        score: SUGGESTED_URL.saturating_sub(order),
                        source: Source::Suggestion(index),
                    });
                }
                None => {
                    let text = folded(suggestion.text.trim());
                    if text == typed || seen.contains(&text) {
                        continue;
                    }
                    let score = suggestion
                        .relevance
                        .map_or(SUGGESTION.saturating_sub(order), |relevance| {
                            SUGGESTION - 99 + relevance.min(1300) * 99 / 1300
                        });
                    seen.push(text);
                    rows.push(Row {
                        kind: MatchKind::Suggestion,
                        score,
                        source: Source::Suggestion(index),
                    });
                }
            }
        }
    }

    fn order(&self, mut rows: Vec<Row<'r>>, inline: bool) -> Vec<Row<'r>> {
        let address = self.request.surface == Surface::AddressField;
        let default = rows
            .iter()
            .enumerate()
            .filter(|(index, row)| {
                !address
                    || matches!(row.kind, MatchKind::TypedUrl | MatchKind::TypedSearch)
                    || (inline && *index == 0)
            })
            .max_by(|(_, a), (_, b)| {
                a.score
                    .cmp(&b.score)
                    .then_with(|| b.kind.rank().cmp(&a.kind.rank()))
            })
            .map(|(index, _)| index);
        let mut ordered = Vec::with_capacity(rows.len());
        if let Some(default) = default {
            ordered.push(rows.remove(default));
        }
        rows.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.kind.rank().cmp(&b.kind.rank()))
        });
        let limits = self.request.limits;
        let mut counts = [0usize; MatchKind::COUNT];
        if let Some(first) = ordered.first() {
            counts[first.kind.index()] += 1;
        }
        for row in rows {
            if ordered.len() >= limits.total_limit() {
                break;
            }
            let count = &mut counts[row.kind.index()];
            if *count >= limits.cap_of(row.kind) {
                continue;
            }
            *count += 1;
            ordered.push(row);
        }
        ordered
    }

    fn to_match(&self, row: Row<'r>) -> Match<K> {
        let (title, url, action, highlight) = match row.source {
            Source::Entry(index) => {
                let entry = &self.candidates.entries[index];
                let shown = entry
                    .url
                    .as_ref()
                    .map(|url| UrlDisplay::steady(url).into_text());
                let title = if entry.title.is_empty() {
                    shown.clone().unwrap_or_default()
                } else {
                    entry.title.to_string()
                };
                (title, shown.unwrap_or_default(), entry_action(entry), true)
            }
            Source::Typed(destination, entry) => {
                let shown = UrlDisplay::steady(destination.url()).into_text();
                let title = entry
                    .map(|index| &self.candidates.entries[index].title)
                    .filter(|title| !title.is_empty())
                    .map_or_else(|| shown.clone(), |title| title.to_string());
                (
                    title,
                    shown,
                    Action::Navigate(destination.url().clone()),
                    false,
                )
            }
            Source::TypedSearch(search) => (
                search.terms().to_owned(),
                String::new(),
                Action::Search(search),
                false,
            ),
            Source::Suggestion(index) => {
                let suggestion = &self.request.suggestions[index];
                match &suggestion.url {
                    Some(url) => {
                        let shown = UrlDisplay::steady(url).into_text();
                        (shown.clone(), shown, Action::Navigate(url.clone()), true)
                    }
                    None => {
                        let engine = self
                            .request
                            .input
                            .search()
                            .and_then(|search| self.request.engines.get(search.engine()))
                            .unwrap_or_else(|| self.request.engines.default_engine());
                        let search = if self.request.input.search().is_some_and(Search::by_keyword)
                        {
                            engine.keyword_search(&suggestion.text)
                        } else {
                            engine.search(&suggestion.text)
                        };
                        (
                            suggestion.text.clone(),
                            String::new(),
                            Action::Search(search),
                            true,
                        )
                    }
                }
            }
            Source::Inline(index, target) => {
                let entry = &self.candidates.entries[index];
                let shown = UrlDisplay::steady(&target).into_text();
                let exact = entry
                    .url
                    .as_ref()
                    .is_some_and(|url| identity(url) == identity(&target));
                let title = if exact && !entry.title.is_empty() {
                    entry.title.to_string()
                } else {
                    shown.clone()
                };
                (title, shown, Action::Navigate(target), true)
            }
        };
        let (title_highlights, url_highlights) = if highlight {
            (
                matching::highlights(&title, self.terms.iter()),
                matching::highlights(&url, self.terms.iter()),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        Match {
            kind: row.kind,
            title,
            url,
            action,
            title_highlights,
            url_highlights,
            relevance: row.score,
        }
    }
}

fn entry_action<K: Clone>(entry: &Entry<K>) -> Action<K> {
    let navigate = || Action::Navigate(entry.url.clone().expect("non-command entries have URLs"));
    match (entry.kind, entry.key.clone()) {
        (MatchKind::OpenTab | MatchKind::Pinned | MatchKind::Favorite, Some(key)) => {
            Action::SwitchToTab(key)
        }
        (MatchKind::Archived, Some(key)) => Action::RestoreArchived(key),
        (MatchKind::Command, Some(key)) => Action::RunCommand(key),
        _ => navigate(),
    }
}

fn bonus(kind: MatchKind) -> f32 {
    match kind {
        MatchKind::OpenTab => 60.0,
        MatchKind::Pinned | MatchKind::Favorite => 50.0,
        MatchKind::Archived => -40.0,
        _ => 0.0,
    }
}

fn command_score(quality: f32, terms: &Terms<'_>, typed: &str, name: &str) -> u32 {
    if typed.trim() == name {
        return COMMAND_EXACT;
    }
    let typed: usize = terms.iter().map(|term| term.chars().count()).sum();
    let length = name.chars().count().max(1);
    let coverage = (typed as f32 / length as f32).min(1.0);
    (COMMAND_BASE + COMMAND_QUALITY * quality * (0.6 + 0.4 * coverage)) as u32
}
