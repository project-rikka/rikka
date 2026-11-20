use std::fmt;

use crate::entry::Entry;
use crate::table::LANGUAGES;

const SIMPLIFIED_CHINESE: &str = "zh-Hans";
const TRADITIONAL_CHINESE: &str = "zh-Hant";
const CANTONESE: &str = "yue";
const LONGEST_TAG: usize = 64;

const ALIASES: &[(&str, &str)] = &[
    ("fa-af", "prs"),
    ("in", "id"),
    ("iw", "he"),
    ("ji", "yi"),
    ("jw", "jv"),
    ("kmr", "ku"),
    ("ku-arab", "ckb"),
    ("lug", "lg"),
    ("mni", "mni-Mtei"),
    ("mww", "hmn"),
    ("nn", "nb"),
    ("no", "nb"),
    ("nya", "ny"),
    ("pa-pk", "pa-Arab"),
    ("pt-br", "pt"),
    ("run", "rn"),
    ("sat-olck", "sat"),
    ("tl", "fil"),
    ("zh-yue", CANTONESE),
];

const CHINESE: &[&str] = &["zh", "cmn"];
const TRADITIONAL_SUBTAGS: &[&str] = &["hant", "tw", "hk", "mo"];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language {
    entry: &'static Entry,
}

impl Language {
    pub fn all() -> impl ExactSizeIterator<Item = Language> + Clone {
        LANGUAGES.iter().map(|entry| Language { entry })
    }

    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Language> {
        LANGUAGES
            .iter()
            .find(|entry| entry.tag.eq_ignore_ascii_case(tag))
            .map(|entry| Language { entry })
    }

    #[must_use]
    pub fn lookup(tag: &str) -> Option<Language> {
        let tag = tag.trim();
        if tag.is_empty()
            || tag.len() > LONGEST_TAG
            || !tag
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return None;
        }
        let tag = tag.replace('_', "-").to_ascii_lowercase();
        let subtags: Vec<&str> = tag.split('-').collect();
        let primary = subtags[0];
        if !(2..=3).contains(&primary.len()) || !primary.bytes().all(|b| b.is_ascii_alphabetic()) {
            return None;
        }
        if CHINESE.contains(&primary) {
            return Language::from_tag(chinese(&subtags[1..]));
        }
        let mut candidates = vec![tag.clone()];
        candidates.extend(
            subtags[1..]
                .iter()
                .map(|subtag| format!("{primary}-{subtag}")),
        );
        candidates.push(primary.to_owned());
        candidates.iter().find_map(|candidate| {
            ALIASES
                .iter()
                .find(|(alias, _)| alias == candidate)
                .and_then(|(_, tag)| Language::from_tag(tag))
                .or_else(|| Language::from_tag(candidate))
        })
    }

    #[must_use]
    pub fn tag(self) -> &'static str {
        self.entry.tag
    }

    #[must_use]
    pub fn english_name(self) -> &'static str {
        self.entry.english
    }

    #[must_use]
    pub fn native_name(self) -> &'static str {
        self.entry.native
    }
}

impl fmt::Debug for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.entry.tag)
    }
}

fn chinese(subtags: &[&str]) -> &'static str {
    if subtags.first() == Some(&CANTONESE) {
        CANTONESE
    } else if subtags
        .iter()
        .any(|subtag| TRADITIONAL_SUBTAGS.contains(subtag))
    {
        TRADITIONAL_CHINESE
    } else {
        SIMPLIFIED_CHINESE
    }
}
