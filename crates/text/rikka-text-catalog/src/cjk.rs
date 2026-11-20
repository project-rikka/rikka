use rikka_text_unicode::Script;

use crate::locale::Locale;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CjkRegion {
    Japan,
    Korea,
    Simplified,
    Traditional,
    HongKong,
}

const SUFFIXES: [(&str, CjkRegion); 5] = [
    ("jp", CjkRegion::Japan),
    ("kr", CjkRegion::Korea),
    ("sc", CjkRegion::Simplified),
    ("tc", CjkRegion::Traditional),
    ("hk", CjkRegion::HongKong),
];

const FAMILY_PREFIXES: [(&str, CjkRegion); 24] = [
    ("yugothic", CjkRegion::Japan),
    ("yumincho", CjkRegion::Japan),
    ("meiryo", CjkRegion::Japan),
    ("msgothic", CjkRegion::Japan),
    ("mspgothic", CjkRegion::Japan),
    ("msuigothic", CjkRegion::Japan),
    ("msmincho", CjkRegion::Japan),
    ("mspmincho", CjkRegion::Japan),
    ("bizud", CjkRegion::Japan),
    ("microsoftyahei", CjkRegion::Simplified),
    ("simsun", CjkRegion::Simplified),
    ("nsimsun", CjkRegion::Simplified),
    ("simhei", CjkRegion::Simplified),
    ("dengxian", CjkRegion::Simplified),
    ("kaiti", CjkRegion::Simplified),
    ("fangsong", CjkRegion::Simplified),
    ("microsoftjhenghei", CjkRegion::Traditional),
    ("mingliu_hkscs", CjkRegion::HongKong),
    ("mingliu", CjkRegion::Traditional),
    ("pmingliu", CjkRegion::Traditional),
    ("malgungothic", CjkRegion::Korea),
    ("gulim", CjkRegion::Korea),
    ("dotum", CjkRegion::Korea),
    ("batang", CjkRegion::Korea),
];

impl CjkRegion {
    pub(crate) fn from_family(folded: &str) -> Option<Self> {
        if folded.contains("cjk") {
            return SUFFIXES
                .iter()
                .find(|(suffix, _)| folded.ends_with(suffix))
                .map(|&(_, region)| region);
        }
        FAMILY_PREFIXES
            .iter()
            .find(|(prefix, _)| folded.starts_with(prefix))
            .map(|&(_, region)| region)
    }

    pub(crate) fn from_locale(locale: &Locale) -> Option<Self> {
        match locale.language() {
            "ja" => Some(Self::Japan),
            "ko" => Some(Self::Korea),
            "yue" => Some(Self::HongKong),
            "zh" => Some(match (locale.script(), locale.region()) {
                (_, Some("HK" | "MO")) => Self::HongKong,
                (Some("Hant"), _) | (_, Some("TW")) => Self::Traditional,
                _ => Self::Simplified,
            }),
            _ => None,
        }
    }

    pub(crate) fn from_script(script: Script) -> Option<Self> {
        match script {
            Script::Han => Some(Self::Simplified),
            Script::Hiragana | Script::Katakana => Some(Self::Japan),
            Script::Hangul => Some(Self::Korea),
            Script::Bopomofo => Some(Self::Traditional),
            _ => None,
        }
    }
}
