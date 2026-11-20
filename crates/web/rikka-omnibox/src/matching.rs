use std::ops::Range;

use crate::fold::fold;
use crate::script::is_cjk;

const OCCURRENCES: usize = 8;

const HOST_START: f32 = 1.0;
const HOST_LABEL: f32 = 0.75;
const MID_HOST: f32 = 0.2;
const PATH_WORD: f32 = 0.6;
const MID_PATH: f32 = 0.2;
const TEXT_START: f32 = 1.0;
const TEXT_WORD: f32 = 0.85;
const CJK_INSIDE: f32 = 0.6;
const MID_TEXT: f32 = 0.3;
const KEYWORD_WEIGHT: f32 = 0.95;

#[derive(Debug)]
pub(crate) struct Terms<'a> {
    terms: Vec<(&'a str, f32)>,
    weight: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Fields<'a> {
    pub(crate) url: &'a str,
    pub(crate) host_start: usize,
    pub(crate) host_end: usize,
    pub(crate) title: &'a str,
    pub(crate) keywords: &'a str,
}

impl<'a> Terms<'a> {
    pub(crate) fn new(folded: &'a str) -> Terms<'a> {
        let terms: Vec<(&str, f32)> = folded
            .split(char::is_whitespace)
            .map(strip_scheme)
            .filter(|term| !term.is_empty())
            .map(|term| (term, term.chars().count() as f32))
            .collect();
        let weight = terms.iter().map(|(_, weight)| weight).sum();
        Terms { terms, weight }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub(crate) fn single(&self) -> Option<&'a str> {
        match self.terms.as_slice() {
            [(term, _)] => Some(term),
            _ => None,
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.terms.iter().map(|(term, _)| *term)
    }

    pub(crate) fn quality(&self, fields: Fields<'_>) -> f32 {
        let mut total = 0.0;
        for &(term, weight) in &self.terms {
            let best = url_score(term, fields)
                .max(text_score(term, fields.title))
                .max(text_score(term, fields.keywords) * KEYWORD_WEIGHT);
            if best <= 0.0 {
                return 0.0;
            }
            total += best * weight;
        }
        total / self.weight
    }
}

fn strip_scheme(term: &str) -> &str {
    term.strip_prefix("https://")
        .or_else(|| term.strip_prefix("http://"))
        .unwrap_or(term)
}

fn best_occurrence(haystack: &str, term: &str, class: impl Fn(usize) -> f32) -> f32 {
    let mut best = 0.0f32;
    let mut consider = |position: usize| {
        best = best.max(class(position));
        best >= 1.0
    };
    let mut characters = term.chars();
    match (characters.next(), characters.next()) {
        (Some(single), None) => {
            for (position, _) in haystack.match_indices(single).take(OCCURRENCES) {
                if consider(position) {
                    break;
                }
            }
        }
        _ => {
            for (position, _) in haystack.match_indices(term).take(OCCURRENCES) {
                if consider(position) {
                    break;
                }
            }
        }
    }
    best
}

fn url_score(term: &str, fields: Fields<'_>) -> f32 {
    best_occurrence(fields.url, term, |position| url_class(fields, position))
}

fn url_class(fields: Fields<'_>, position: usize) -> f32 {
    if position == fields.host_start || (position == 0 && fields.host_end > 0) {
        return HOST_START;
    }
    let previous = fields.url[..position].chars().next_back();
    if position < fields.host_end {
        return if previous == Some('.') {
            HOST_LABEL
        } else {
            MID_HOST
        };
    }
    let current = fields.url[position..].chars().next();
    if is_boundary(previous, current) {
        PATH_WORD
    } else if current.is_some_and(is_cjk) {
        CJK_INSIDE
    } else {
        MID_PATH
    }
}

fn text_score(term: &str, text: &str) -> f32 {
    if text.len() < term.len() {
        return 0.0;
    }
    best_occurrence(text, term, |position| text_class(text, position))
}

fn text_class(text: &str, position: usize) -> f32 {
    if position == 0 {
        return TEXT_START;
    }
    let previous = text[..position].chars().next_back();
    let current = text[position..].chars().next();
    if is_boundary(previous, current) {
        TEXT_WORD
    } else if current.is_some_and(is_cjk) {
        CJK_INSIDE
    } else {
        MID_TEXT
    }
}

fn is_boundary(previous: Option<char>, current: Option<char>) -> bool {
    match (previous, current) {
        (None, _) => true,
        (Some(previous), Some(current)) => {
            !previous.is_alphanumeric() || is_cjk(previous) != is_cjk(current)
        }
        (Some(_), None) => false,
    }
}

pub(crate) fn highlights<'t>(
    text: &str,
    terms: impl Iterator<Item = &'t str>,
) -> Vec<Range<usize>> {
    let mut folded = String::with_capacity(text.len());
    let mut original = Vec::with_capacity(text.len() + 1);
    for (at, character) in text.char_indices() {
        let character = fold(character);
        folded.push(character);
        original.extend(std::iter::repeat_n(at, character.len_utf8()));
    }
    original.push(text.len());
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for term in terms {
        let best = folded
            .match_indices(term)
            .take(OCCURRENCES)
            .map(|(position, _)| (text_class(&folded, position), position))
            .fold(None, |best: Option<(f32, usize)>, candidate| match best {
                Some(best) if best.0 >= candidate.0 => Some(best),
                _ => Some(candidate),
            });
        if let Some((_, position)) = best {
            ranges.push(original[position]..original[position + term.len()]);
        }
    }
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}
