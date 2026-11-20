use std::ops::Range;
use std::sync::Arc;

use rikka_fuzzy::{Haystack, Needle, Score, byte_ranges};
use rikka_text_document::Rope;
use rustc_hash::FxHashMap;
use tree_sitter as ts;

use crate::language::Language;
use crate::suggestion::{Kind, Suggestion};
use crate::words::{is_word, is_word_shaped};

const WINDOW_RADIUS: usize = 256 * 1024;
const MAX_WORD_LEN: usize = 96;
const MAX_NODES: usize = 600_000;
const MAX_WORDS: usize = 40_000;

const SKIPPED_KINDS: [&str; 10] = [
    "comment", "string", "number", "integer", "float", "escape", "literal", "content", "text",
    "fragment",
];

#[derive(Clone, Debug)]
struct Entry {
    word: Arc<str>,
    kind: Kind,
    distance: usize,
    occurrences: u32,
}

#[derive(Default)]
pub struct Lexicon {
    entries: Vec<Entry>,
}

impl std::fmt::Debug for Lexicon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lexicon")
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl Lexicon {
    #[must_use]
    pub fn from_text(rope: &Rope, at: usize, language: Option<Language>) -> Lexicon {
        let window = window(rope, at);
        let mut gathering = WordScan::new(at);
        let text = rope.slice(window.clone());
        let mut start: Option<usize> = None;
        for (offset, letter) in text.char_indices().chain([(text.len(), ' ')]) {
            match (start, is_word(language, letter)) {
                (None, true) => start = Some(offset),
                (Some(began), false) => {
                    start = None;
                    let range = window.start + began..window.start + offset;
                    gathering.take(&text[began..offset], range, Kind::Word, language);
                }
                _ => {}
            }
        }
        gathering.finish(language)
    }

    pub(crate) fn from_tree<'tree>(
        root: ts::Node<'tree>,
        rope: &Rope,
        at: usize,
        language: Language,
    ) -> Lexicon {
        let window = window(rope, at);
        let mut gathering = WordScan::new(at);
        let mut ancestors: Vec<&'tree str> = Vec::new();
        let mut cursor = root.walk();
        let mut visited = 0usize;
        loop {
            visited += 1;
            if visited > MAX_NODES {
                break;
            }
            let node = cursor.node();
            let overlaps = node.end_byte() > window.start && node.start_byte() < window.end;
            if overlaps && node.child_count() > 0 && cursor.goto_first_child() {
                ancestors.push(node.kind());
                continue;
            }
            if overlaps && node.child_count() == 0 && node.is_named() {
                let field = cursor.field_name();
                let parent = ancestors.last().copied();
                gathering.leaf(rope, node, parent, field, language);
            }
            if !advance(&mut cursor, &mut ancestors, window.end) {
                break;
            }
        }
        if gathering.words.is_empty() {
            return Lexicon::from_text(rope, at, Some(language));
        }
        gathering.finish(Some(language))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn suggest(&self, typed: &str, most: usize) -> Vec<Suggestion> {
        if most == 0 {
            return Vec::new();
        }
        if typed.is_empty() {
            return self
                .entries
                .iter()
                .filter(|entry| entry.kind != Kind::Keyword)
                .take(most)
                .map(|entry| Suggestion::new(Arc::clone(&entry.word), entry.kind, Vec::new()))
                .collect();
        }

        let needle = Needle::with_smart_case(typed);
        let mut positions = Vec::new();
        let mut ranked: Vec<(Score, usize, Vec<usize>)> = Vec::new();
        for (at, entry) in self.entries.iter().enumerate() {
            if *entry.word == *typed {
                continue;
            }
            let Some(score) = needle.positions(&Haystack::new(&entry.word), &mut positions) else {
                continue;
            };
            if !positions
                .first()
                .is_some_and(|&first| begins_word(&entry.word, first))
            {
                continue;
            }
            ranked.push((score, at, positions.clone()));
        }
        ranked.sort_by(|one, other| {
            let (left, right) = (&self.entries[one.1], &self.entries[other.1]);
            other
                .0
                .cmp(&one.0)
                .then(left.distance.cmp(&right.distance))
                .then(right.occurrences.cmp(&left.occurrences))
                .then(left.word.len().cmp(&right.word.len()))
                .then(left.word.cmp(&right.word))
        });
        ranked
            .into_iter()
            .take(most)
            .map(|(_, at, positions)| {
                let entry = &self.entries[at];
                let highlighted_ranges = byte_ranges(&entry.word, &positions);
                Suggestion::new(Arc::clone(&entry.word), entry.kind, highlighted_ranges)
            })
            .collect()
    }
}

struct WordScan {
    at: usize,
    words: FxHashMap<String, Entry>,
}

impl WordScan {
    fn new(at: usize) -> WordScan {
        WordScan {
            at,
            words: FxHashMap::default(),
        }
    }

    fn leaf(
        &mut self,
        rope: &Rope,
        node: ts::Node<'_>,
        parent: Option<&str>,
        field: Option<&str>,
        language: Language,
    ) {
        let kind = node.kind();
        if SKIPPED_KINDS.iter().any(|skipped| kind.contains(skipped)) {
            return;
        }
        let range = node.byte_range();
        if range.is_empty() || range.len() > MAX_WORD_LEN || range.end > rope.len() {
            return;
        }
        let word = rope.slice(range.clone());
        let sort = kind_of(kind, parent, field, &word);
        self.take(&word, range, sort, Some(language));
    }

    fn take(&mut self, word: &str, range: Range<usize>, kind: Kind, language: Option<Language>) {
        if range.start < self.at && self.at <= range.end {
            return;
        }
        if word.chars().count() < 2 || word.len() > MAX_WORD_LEN || !is_word_shaped(language, word)
        {
            return;
        }
        let distance = match range.end <= self.at {
            true => self.at - range.end,
            false => range.start - self.at,
        };
        if let Some(entry) = self.words.get_mut(word) {
            entry.distance = entry.distance.min(distance);
            entry.occurrences = entry.occurrences.saturating_add(1);
            entry.kind = stronger_kind(entry.kind, kind);
            return;
        }
        if self.words.len() >= MAX_WORDS {
            return;
        }
        self.words.insert(
            word.to_owned(),
            Entry {
                word: Arc::from(word),
                kind,
                distance,
                occurrences: 1,
            },
        );
    }

    fn finish(mut self, language: Option<Language>) -> Lexicon {
        let keywords = language
            .and_then(Language::rules)
            .map(|rules| &*rules.keywords)
            .unwrap_or_default();
        for keyword in keywords {
            if let Some(entry) = self.words.get_mut(&**keyword) {
                entry.kind = Kind::Keyword;
                continue;
            }
            self.words.insert(
                keyword.to_string(),
                Entry {
                    word: Arc::from(&**keyword),
                    kind: Kind::Keyword,
                    distance: usize::MAX,
                    occurrences: 0,
                },
            );
        }
        let mut entries: Vec<Entry> = self.words.into_values().collect();
        entries.sort_by(|one, other| {
            one.distance
                .cmp(&other.distance)
                .then(other.occurrences.cmp(&one.occurrences))
                .then(one.word.cmp(&other.word))
        });
        Lexicon { entries }
    }
}

fn advance<'tree>(
    cursor: &mut ts::TreeCursor<'tree>,
    ancestors: &mut Vec<&'tree str>,
    end: usize,
) -> bool {
    loop {
        if cursor.goto_next_sibling() && cursor.node().start_byte() < end {
            return true;
        }
        if !cursor.goto_parent() {
            return false;
        }
        ancestors.pop();
    }
}

fn window(rope: &Rope, at: usize) -> Range<usize> {
    let length = rope.len();
    let at = at.min(length);
    let start = rope.line_start(rope.line_of(at.saturating_sub(WINDOW_RADIUS)));
    let end = (at + WINDOW_RADIUS).min(length);
    let end = match end == length {
        true => length,
        false => rope.line_start(rope.line_of(end)),
    };
    start..end.max(at)
}

pub(crate) fn kind_of(leaf: &str, parent: Option<&str>, field: Option<&str>, word: &str) -> Kind {
    let parent = parent.unwrap_or_default();
    let named = matches!(field, Some("name"));
    if leaf.contains("type") || (named && declares_type(parent)) {
        return Kind::Type;
    }
    if leaf.contains("field")
        || leaf.contains("property")
        || matches!(field, Some("field" | "property" | "attribute" | "key"))
    {
        return Kind::Field;
    }
    if matches!(field, Some("name" | "function" | "method" | "macro")) && calls(parent) {
        return Kind::Function;
    }
    if matches!(field, Some("path")) || (named && declares_module(parent)) {
        return Kind::Module;
    }
    if leaf == "constant" || is_shouting(word) {
        return Kind::Constant;
    }
    if is_capitalised(word) {
        return Kind::Type;
    }
    Kind::Variable
}

fn declares_type(parent: &str) -> bool {
    [
        "class",
        "struct",
        "enum",
        "interface",
        "trait",
        "union",
        "type",
    ]
    .iter()
    .any(|kind| parent.contains(kind))
}

fn calls(parent: &str) -> bool {
    ["function", "method", "call", "macro", "def", "lambda"]
        .iter()
        .any(|kind| parent.contains(kind))
}

fn declares_module(parent: &str) -> bool {
    ["mod_item", "module", "namespace", "package"]
        .iter()
        .any(|kind| parent.contains(kind))
}

fn is_shouting(word: &str) -> bool {
    word.chars().any(char::is_uppercase) && !word.chars().any(char::is_lowercase)
}

fn is_capitalised(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase) && word.chars().any(char::is_lowercase)
}

fn stronger_kind(current: Kind, occurrences: Kind) -> Kind {
    match (current, occurrences) {
        (Kind::Variable | Kind::Word, other) => other,
        (current, _) => current,
    }
}

fn begins_word(word: &str, index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let mut letters = word.chars().skip(index - 1);
    match (letters.next(), letters.next()) {
        (Some(before), Some(here)) => {
            !before.is_alphanumeric() || (before.is_lowercase() && here.is_uppercase())
        }
        _ => false,
    }
}
