use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use rikka_i18n::{Locale, current_locale, negotiate};

use crate::group::Group;
use crate::tone::Tone;

const TABLE: &str = include_str!("../data/emoji.txt");

const ANNOTATIONS: &[(&str, &str)] = &[
    ("en", include_str!("../data/en.txt")),
    ("ja", include_str!("../data/ja.txt")),
    ("zh-Hans", include_str!("../data/zh-Hans.txt")),
];

const ENGLISH: &str = "en";

const TONES: usize = 5;

const EXACT_NAME: u32 = 4;
const NAME_PREFIX: u32 = 3;
const EXACT_KEYWORD: u32 = 2;
const NAME_SUBSTRING: u32 = 1;
const KEYWORD_SUBSTRING: u32 = 0;

#[derive(Debug)]
pub struct Emoji {
    pub text: &'static str,
    pub group: Group,
    pub name: &'static str,
    pub code: String,
    tones: Option<[&'static str; TONES]>,
    folded: String,
    keywords: String,
}

impl Emoji {
    #[must_use]
    pub fn with_tone(&self, tone: Tone) -> &'static str {
        let Some(slot) = tone.slot() else {
            return self.text;
        };
        self.tones.map_or(self.text, |tones| tones[slot])
    }

    #[must_use]
    pub fn supports_tone(&self) -> bool {
        self.tones.is_some()
    }

    fn score(&self, word: &str) -> Option<u32> {
        if self.folded == word {
            return Some(EXACT_NAME);
        }
        if self.folded.starts_with(word) {
            return Some(NAME_PREFIX);
        }
        if self.keywords.split('\t').any(|keyword| keyword == word) {
            return Some(EXACT_KEYWORD);
        }
        if self.folded.contains(word) {
            return Some(NAME_SUBSTRING);
        }
        self.keywords.contains(word).then_some(KEYWORD_SUBSTRING)
    }
}

#[derive(Debug)]
pub struct Catalog {
    emoji: Vec<Emoji>,
    index_by_text: HashMap<&'static str, usize>,
    sample: Option<usize>,
}

impl Catalog {
    #[must_use]
    pub fn current() -> Arc<Catalog> {
        static CACHED: Mutex<Option<(Locale, Arc<Catalog>)>> = Mutex::new(None);
        let locale = catalog_locale();
        let mut cached = CACHED.lock();
        if let Some((cached_locale, catalog)) = &*cached
            && *cached_locale == locale
        {
            return Arc::clone(catalog);
        }
        let catalog = Arc::new(Catalog::read(&locale));
        *cached = Some((locale, Arc::clone(&catalog)));
        catalog
    }

    #[must_use]
    pub fn read(locale: &Locale) -> Catalog {
        let tag = locale.to_string();
        let english = annotations_for(ENGLISH);
        let localized = match tag == ENGLISH {
            true => HashMap::new(),
            false => annotations_for(&tag),
        };

        let emoji: Vec<Emoji> = table_rows(TABLE)
            .filter_map(|fields| {
                let text = *fields.first()?;
                let group = Group::from_slug(fields.get(1)?)?;
                let annotations = [localized.get(text), english.get(text)];
                let name = annotations
                    .iter()
                    .flatten()
                    .next()
                    .map_or(text, |annotation| annotation.name);
                let code = english
                    .get(text)
                    .map(|annotation| shortcode(annotation.name))
                    .unwrap_or_default();
                let mut keywords: Vec<&str> = annotations
                    .iter()
                    .flatten()
                    .map(|annotation| annotation.keywords)
                    .collect();
                keywords.push(&code);
                let keywords = folded(&keywords.join("\t"));
                Some(Emoji {
                    text,
                    group,
                    name,
                    tones: parse_tones(fields.get(2).copied()),
                    folded: folded(name),
                    keywords,
                    code,
                })
            })
            .collect();

        let index_by_text: HashMap<&'static str, usize> = emoji
            .iter()
            .enumerate()
            .map(|(index, emoji)| (emoji.text, index))
            .collect();
        let sample = index_by_text.get(Tone::SAMPLE).copied();
        Catalog {
            emoji,
            index_by_text,
            sample,
        }
    }

    #[must_use]
    pub fn sample(&self, tone: Tone) -> &'static str {
        self.sample
            .and_then(|index| self.get(index))
            .map_or(Tone::SAMPLE, |emoji| emoji.with_tone(tone))
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Emoji> {
        self.emoji.get(index)
    }

    #[must_use]
    pub fn position(&self, text: &str) -> Option<usize> {
        self.index_by_text.get(text).copied()
    }

    #[must_use]
    pub fn group_indices(&self, group: Group) -> Vec<usize> {
        self.emoji
            .iter()
            .enumerate()
            .filter(|(_, emoji)| emoji.group == group)
            .map(|(index, _)| index)
            .collect()
    }

    #[must_use]
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = folded(query.trim());
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }

        let mut ranked: Vec<(usize, u32)> = self
            .emoji
            .iter()
            .enumerate()
            .filter_map(|(index, emoji)| {
                let mut total = 0;
                for word in &words {
                    total += emoji.score(word)?;
                }
                Some((index, total))
            })
            .collect();
        ranked.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        ranked.into_iter().map(|(index, _)| index).collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct Annotation {
    name: &'static str,
    keywords: &'static str,
}

fn annotations_for(tag: &str) -> HashMap<&'static str, Annotation> {
    let Some((_, document)) = ANNOTATIONS.iter().find(|(language, _)| *language == tag) else {
        return HashMap::new();
    };
    document
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (text, rest) = line.split_once('\t')?;
            let (name, keywords) = rest.split_once('\t').unwrap_or((rest, ""));
            Some((text, Annotation { name, keywords }))
        })
        .collect()
}

fn shortcode(name: &str) -> String {
    let mut code = String::with_capacity(name.len());
    for letter in name.to_lowercase().chars() {
        match letter {
            'a'..='z' | '0'..='9' => code.push(letter),
            _ if !code.ends_with('_') && !code.is_empty() => code.push('_'),
            _ => {}
        }
    }
    code.trim_end_matches('_').to_owned()
}

fn folded(text: &str) -> String {
    text.to_lowercase().chars().map(to_katakana).collect()
}

fn to_katakana(character: char) -> char {
    const FIRST: char = '\u{3041}';
    const LAST: char = '\u{3096}';
    const SHIFT: u32 = 0x60;

    match (FIRST..=LAST).contains(&character) {
        true => char::from_u32(character as u32 + SHIFT).unwrap_or(character),
        false => character,
    }
}

fn table_rows(document: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    document
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').collect())
}

fn parse_tones(tones: Option<&'static str>) -> Option<[&'static str; TONES]> {
    let listed: Vec<&'static str> = tones?.split(' ').collect();
    listed.try_into().ok()
}

fn catalog_locale() -> Locale {
    let available: Vec<Locale> = ANNOTATIONS
        .iter()
        .filter_map(|(tag, _)| tag.parse().ok())
        .collect();
    let requested = current_locale();
    negotiate([&requested], &available)
        .unwrap_or_else(|| ENGLISH.parse().expect("English locale tag parses"))
}
