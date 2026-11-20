use rikka_text_font::Tag;
use rikka_text_unicode::Script;

use crate::common::language::Language;
use crate::tables::language_tags::LANGUAGE_TAGS;

const CAPACITY: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TagList {
    tags: [Tag; CAPACITY],
    len: usize,
}

impl TagList {
    pub(crate) fn push(&mut self, tag: Tag) {
        if self.len < CAPACITY {
            self.tags[self.len] = tag;
            self.len += 1;
        }
    }

    pub(crate) fn extend(&mut self, tags: &[Tag]) {
        for &tag in tags {
            self.push(tag);
        }
    }

    pub(crate) fn as_slice(&self) -> &[Tag] {
        &self.tags[..self.len]
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OpenTypeTags {
    pub(crate) scripts: TagList,
    pub(crate) languages: TagList,
}

impl OpenTypeTags {
    pub(crate) fn new(script: Script, language: Option<&Language>) -> Self {
        let mut tags = Self::default();
        let mut needs_script = true;
        if let Some(language) = language {
            let text = language.as_str();
            let (prefix, private) = split_private_use(text);
            if let Some(tag) = private.and_then(|private| private_tag(private, "-hbsc", false)) {
                tags.scripts.push(tag);
                needs_script = false;
            }
            match private.and_then(|private| private_tag(private, "-hbot", true)) {
                Some(tag) => tags.languages.push(tag),
                None if !prefix.is_empty() => language_tags(prefix, &mut tags.languages),
                None => {}
            }
        }
        if needs_script {
            for tag in script.opentype_tags() {
                tags.scripts.push(Tag::new(tag));
            }
        }
        tags
    }
}

fn split_private_use(language: &str) -> (&str, Option<&str>) {
    if language.starts_with("x-") {
        return ("", Some(language));
    }
    match language.find("-x-") {
        Some(at) => (&language[..at], Some(&language[at + 1..])),
        None => (language, None),
    }
}

fn private_tag(private: &str, marker: &str, uppercase: bool) -> Option<Tag> {
    let at = private.find(marker)?;
    let rest = &private.as_bytes()[at + marker.len()..];
    let length = rest
        .iter()
        .take(4)
        .take_while(|byte| byte.is_ascii_alphanumeric())
        .count();
    if length == 0 {
        return None;
    }
    let mut bytes = [b' '; 4];
    for (target, &byte) in bytes.iter_mut().zip(&rest[..length]) {
        *target = if uppercase {
            byte.to_ascii_uppercase()
        } else {
            byte.to_ascii_lowercase()
        };
    }
    let tag = Tag::new(&bytes);
    if tag.to_u32() & 0xDFDF_DFDF == u32::from_be_bytes(*b"DFLT") {
        return Some(Tag::from_u32(tag.to_u32() ^ !0xDFDF_DFDF));
    }
    Some(tag)
}

fn has_subtag(language: &str, subtag: &str) -> bool {
    language.split('-').skip(1).any(|part| part == subtag)
}

fn complex_language_tags(language: &str, tags: &mut TagList) -> bool {
    const VARIANTS: [(&str, &[u8; 4]); 9] = [
        ("fonnapa", b"APPH"),
        ("polyton", b"PGR "),
        ("arevmda", b"HYE "),
        ("provenc", b"PRO "),
        ("fonipa", b"IPPH"),
        ("geok", b"KGE "),
        ("syre", b"SYRE"),
        ("syrj", b"SYRJ"),
        ("syrn", b"SYRN"),
    ];
    if let Some((_, tag)) = VARIANTS
        .iter()
        .find(|(subtag, _)| has_subtag(language, subtag))
    {
        tags.push(Tag::new(tag));
        return true;
    }
    let primary = language.split('-').next().unwrap_or(language);
    if is_chinese(primary) && language.len() > primary.len() {
        let hant = has_subtag(language, "hant");
        let selected: &[&[u8; 4]] = if hant && has_subtag(language, "hk") {
            &[b"ZHH "]
        } else if hant && has_subtag(language, "mo") {
            &[b"ZHTM", b"ZHH "]
        } else if has_subtag(language, "hans") {
            &[b"ZHS "]
        } else if hant {
            &[b"ZHT "]
        } else if has_subtag(language, "hk") {
            &[b"ZHH "]
        } else if has_subtag(language, "mo") {
            &[b"ZHTM", b"ZHH "]
        } else if has_subtag(language, "tw") {
            &[b"ZHT "]
        } else {
            &[]
        };
        for tag in selected {
            tags.push(Tag::new(tag));
        }
        return !selected.is_empty();
    }
    if primary == "ro" && has_subtag(language, "md") {
        tags.push(Tag::new(b"MOL "));
        tags.push(Tag::new(b"ROM "));
        return true;
    }
    false
}

fn is_chinese(primary: &str) -> bool {
    matches!(
        primary,
        "zh" | "zho"
            | "cdo"
            | "cjy"
            | "cmn"
            | "cnp"
            | "cpx"
            | "csp"
            | "czh"
            | "czo"
            | "gan"
            | "hak"
            | "hnm"
            | "hsn"
            | "lzh"
            | "mnp"
            | "nan"
            | "sjc"
            | "wuu"
            | "yue"
    )
}

fn language_tags(language: &str, tags: &mut TagList) {
    if complex_language_tags(language, tags) {
        return;
    }
    let mut parts = language.split('-');
    let primary = parts.next().unwrap_or(language);
    let extended = parts
        .next()
        .filter(|part| part.len() == 3 && part.bytes().all(|byte| byte.is_ascii_alphabetic()));
    let key = extended.unwrap_or(primary);
    if let Ok(index) = LANGUAGE_TAGS.binary_search_by(|(candidate, _)| candidate.cmp(&key)) {
        tags.extend(LANGUAGE_TAGS[index].1);
        return;
    }
    if language.len() == 3 {
        let bytes = language.as_bytes();
        tags.push(Tag::new(&[
            bytes[0].to_ascii_uppercase(),
            bytes[1].to_ascii_uppercase(),
            bytes[2].to_ascii_uppercase(),
            b' ',
        ]));
    }
}
