use rikka_text_unicode::Script;

use crate::cjk::CjkRegion;
use crate::config::{Alias, Aliases};
use crate::family::Family;

const SEGOE_UI: &[&str] = &["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"];
const JAPANESE: &[&str] = &["Yu Gothic UI", "Meiryo UI", "MS UI Gothic"];
const KOREAN: &[&str] = &["Malgun Gothic"];
const SIMPLIFIED_CHINESE: &[&str] = &["Microsoft YaHei UI"];
const TRADITIONAL_CHINESE: &[&str] = &["Microsoft JhengHei UI"];
const INDIC: &[&str] = &["Nirmala UI"];
const SOUTHEAST_ASIAN: &[&str] = &["Leelawadee UI"];
const SYMBOLS: &[&str] = &[
    "Segoe UI Variable Text",
    "Segoe UI Variable",
    "Segoe UI",
    "Segoe UI Symbol",
];
const OTHERS: &[&str] = &[
    "Segoe UI Historic",
    "Ebrima",
    "Gadugi",
    "Myanmar Text",
    "Javanese Text",
    "Mongolian Baiti",
    "Segoe UI Symbol",
];

const ALIASES: [(&str, &[&str]); 9] = [
    ("Noto Sans", SEGOE_UI),
    ("Noto Serif", &["Cambria", "Times New Roman"]),
    ("Noto Sans Mono", &["Cascadia Mono", "Consolas"]),
    ("Noto Color Emoji", &["Segoe UI Emoji"]),
    ("Noto Sans CJK JP", JAPANESE),
    ("Noto Sans CJK KR", KOREAN),
    ("Noto Sans CJK SC", SIMPLIFIED_CHINESE),
    ("Noto Sans CJK TC", TRADITIONAL_CHINESE),
    ("Noto Sans CJK HK", TRADITIONAL_CHINESE),
];

pub(crate) fn preferred_families(generic: &Family) -> &'static [&'static str] {
    match generic {
        Family::SansSerif | Family::SystemUi => &[
            "Inter",
            "Segoe UI Variable Text",
            "Segoe UI Variable",
            "Segoe UI",
        ],
        Family::Monospace => &["JetBrains Mono", "Cascadia Mono", "Consolas"],
        Family::Serif => &["Cambria", "Times New Roman"],
        Family::Emoji => &["Segoe UI Emoji"],
        Family::Cursive => &["Comic Sans MS"],
        Family::Fantasy => &["Impact"],
        Family::Math => &["Cambria Math"],
        Family::Named(_) => &[],
    }
}

pub(crate) fn fallback_families(
    script: Script,
    region: Option<CjkRegion>,
) -> &'static [&'static str] {
    if let Some(region) = region.filter(|_| CjkRegion::from_script(script).is_some()) {
        return match region {
            CjkRegion::Japan => JAPANESE,
            CjkRegion::Korea => KOREAN,
            CjkRegion::Simplified => SIMPLIFIED_CHINESE,
            CjkRegion::Traditional | CjkRegion::HongKong => TRADITIONAL_CHINESE,
        };
    }
    match script {
        Script::Common | Script::Inherited | Script::Unknown => SYMBOLS,
        Script::Latin
        | Script::Greek
        | Script::Cyrillic
        | Script::Armenian
        | Script::Georgian
        | Script::Arabic
        | Script::Hebrew => SEGOE_UI,
        Script::Devanagari
        | Script::Bengali
        | Script::Gurmukhi
        | Script::Gujarati
        | Script::Oriya
        | Script::Tamil
        | Script::Telugu
        | Script::Kannada
        | Script::Malayalam
        | Script::Sinhala
        | Script::OlChiki => INDIC,
        Script::Thai | Script::Lao | Script::Khmer | Script::Buginese => SOUTHEAST_ASIAN,
        Script::Ethiopic | Script::Nko | Script::Tifinagh | Script::Vai => &["Ebrima"],
        Script::Cherokee | Script::CanadianAboriginal | Script::Osage => &["Gadugi"],
        Script::Myanmar => &["Myanmar Text"],
        Script::Javanese => &["Javanese Text"],
        Script::Mongolian => &["Mongolian Baiti"],
        Script::Tibetan => &["Microsoft Himalaya"],
        Script::Yi => &["Microsoft Yi Baiti"],
        Script::TaiLe => &["Microsoft Tai Le"],
        Script::NewTaiLue => &["Microsoft New Tai Lue"],
        Script::PhagsPa => &["Microsoft PhagsPa"],
        Script::Thaana => &["MV Boli"],
        _ => OTHERS,
    }
}

pub(crate) fn aliases() -> Aliases {
    let mut aliases = Aliases::default();
    for (family, accept) in ALIASES {
        aliases.insert(
            family,
            Alias {
                accept: accept.iter().map(|&name| name.to_owned()).collect(),
                ..Alias::default()
            },
        );
    }
    aliases
}
