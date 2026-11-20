use rikka_text_unicode::Script;
use url::quirks::domain_to_unicode;

use crate::script::{Scripts, is_kana, script};

const DENIED: &[(char, char)] = &[
    ('\u{00B7}', '\u{00B7}'),
    ('\u{00DF}', '\u{00DF}'),
    ('\u{0138}', '\u{0138}'),
    ('\u{0185}', '\u{0185}'),
    ('\u{01C0}', '\u{01C3}'),
    ('\u{0261}', '\u{0261}'),
    ('\u{0269}', '\u{026A}'),
    ('\u{0294}', '\u{0294}'),
    ('\u{0296}', '\u{0297}'),
    ('\u{02B9}', '\u{02DF}'),
    ('\u{0335}', '\u{0338}'),
    ('\u{03C2}', '\u{03C2}'),
    ('\u{0589}', '\u{0589}'),
    ('\u{05BE}', '\u{05BE}'),
    ('\u{05C3}', '\u{05C3}'),
    ('\u{05F4}', '\u{05F4}'),
    ('\u{0609}', '\u{060A}'),
    ('\u{066A}', '\u{066A}'),
    ('\u{06D4}', '\u{06D4}'),
    ('\u{0701}', '\u{070D}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{1735}', '\u{1735}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{2010}', '\u{2015}'),
    ('\u{2027}', '\u{2027}'),
    ('\u{2039}', '\u{203A}'),
    ('\u{2043}', '\u{2044}'),
    ('\u{2060}', '\u{206F}'),
    ('\u{2212}', '\u{2212}'),
    ('\u{2215}', '\u{2216}'),
    ('\u{2236}', '\u{2236}'),
    ('\u{23AE}', '\u{23AE}'),
    ('\u{2571}', '\u{2572}'),
    ('\u{2800}', '\u{2800}'),
    ('\u{29F6}', '\u{29F6}'),
    ('\u{29F8}', '\u{29F8}'),
    ('\u{2AFB}', '\u{2AFB}'),
    ('\u{2AFD}', '\u{2AFD}'),
    ('\u{2E3A}', '\u{2E3B}'),
    ('\u{2F00}', '\u{2FDF}'),
    ('\u{3014}', '\u{3015}'),
    ('\u{3033}', '\u{3035}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{321D}', '\u{321E}'),
    ('\u{33AE}', '\u{33AF}'),
    ('\u{33C6}', '\u{33C6}'),
    ('\u{33DF}', '\u{33DF}'),
    ('\u{A789}', '\u{A789}'),
    ('\u{FE14}', '\u{FE15}'),
    ('\u{FE3F}', '\u{FE3F}'),
    ('\u{FE5D}', '\u{FE5E}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FF0E}', '\u{FF0F}'),
    ('\u{FF61}', '\u{FF61}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{1F50F}', '\u{1F513}'),
];

const CYRILLIC_LOOKALIKES: &str = "абгезопрсухъьѕіјӏԁԗԛԝҽѵѡһ";
const CYRILLIC_TLDS: &[&str] = &[
    "bg",
    "by",
    "kg",
    "kz",
    "mk",
    "mn",
    "rs",
    "ru",
    "su",
    "tj",
    "ua",
    "uz",
    "бг",
    "бел",
    "дети",
    "ком",
    "қаз",
    "мкд",
    "мон",
    "москва",
    "онлайн",
    "орг",
    "рус",
    "рф",
    "сайт",
    "срб",
    "укр",
];
const GREEK_LOOKALIKES: &str = "αγικνορυχω";
const GREEK_TLDS: &[&str] = &["cy", "gr", "ελ", "ευ"];
const SINGLE_KATAKANA_LOOKALIKES: &str = "ノンソゾ";

const JAPANESE: Scripts = Scripts::of(&[
    Script::Latin,
    Script::Han,
    Script::Hiragana,
    Script::Katakana,
]);
const CHINESE: Scripts = Scripts::of(&[Script::Latin, Script::Han, Script::Bopomofo]);
const KOREAN: Scripts = Scripts::of(&[Script::Latin, Script::Han, Script::Hangul]);
const CYRILLIC: Scripts = Scripts::of(&[Script::Cyrillic]);
const GREEK: Scripts = Scripts::of(&[Script::Greek]);

pub(crate) fn display_host(host: &str) -> String {
    if !host.split('.').any(is_ace) {
        return host.to_owned();
    }
    let top = host
        .trim_end_matches('.')
        .rsplit('.')
        .next()
        .unwrap_or_default();
    let top = decode_label(top).unwrap_or_else(|| top.to_owned());
    let mut shown = String::with_capacity(host.len());
    for (index, label) in host.split('.').enumerate() {
        if index > 0 {
            shown.push('.');
        }
        match decode_label(label) {
            Some(unicode) if is_safe_label(&unicode, &top) => shown.push_str(&unicode),
            _ => shown.push_str(label),
        }
    }
    shown
}

fn is_ace(label: &str) -> bool {
    label
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("xn--"))
}

fn decode_label(label: &str) -> Option<String> {
    if !is_ace(label) {
        return None;
    }
    let unicode = domain_to_unicode(label);
    let decoded = !unicode.is_empty() && !is_ace(&unicode) && !unicode.contains('\u{FFFD}');
    decoded.then_some(unicode)
}

fn is_denied(character: char) -> bool {
    DENIED
        .iter()
        .any(|&(start, end)| (start..=end).contains(&character))
}

pub(crate) fn is_safe_label(label: &str, top: &str) -> bool {
    let characters: Vec<char> = label.chars().collect();
    if characters
        .iter()
        .any(|&character| is_denied(character) || character.is_control())
    {
        return false;
    }
    let mut scripts = Scripts::default();
    for &character in &characters {
        let written = script(character);
        let neutral = matches!(written, Script::Common | Script::Inherited);
        if !neutral && !scripts.add(written) {
            return false;
        }
    }
    allowed_mix(scripts)
        && !mixes_digits(&characters)
        && !marks_latin(&characters)
        && !whole_script_confusable(&characters, scripts, top)
        && japanese_safe(&characters)
}

fn allowed_mix(scripts: Scripts) -> bool {
    scripts.count() <= 1
        || scripts.within(JAPANESE)
        || scripts.within(CHINESE)
        || scripts.within(KOREAN)
}

fn mixes_digits(characters: &[char]) -> bool {
    let ascii = characters.iter().any(char::is_ascii_digit);
    let other = characters
        .iter()
        .any(|&character| !character.is_ascii() && character.is_numeric());
    ascii && other
}

fn marks_latin(characters: &[char]) -> bool {
    characters.windows(2).any(|pair| {
        script(pair[1]) == Script::Inherited
            && matches!(script(pair[0]), Script::Latin | Script::Inherited)
    })
}

fn whole_script_confusable(characters: &[char], scripts: Scripts, top: &str) -> bool {
    let (lookalikes, tlds) = if scripts == CYRILLIC {
        (CYRILLIC_LOOKALIKES, CYRILLIC_TLDS)
    } else if scripts == GREEK {
        (GREEK_LOOKALIKES, GREEK_TLDS)
    } else {
        return false;
    };
    !tlds.contains(&top)
        && characters.iter().all(|&character| {
            character.is_ascii_digit() || character == '-' || lookalikes.contains(character)
        })
}

fn japanese_safe(characters: &[char]) -> bool {
    if characters
        .iter()
        .all(|&character| SINGLE_KATAKANA_LOOKALIKES.contains(character))
    {
        return false;
    }
    let katakana =
        |character: Option<&char>| character.is_some_and(|&c| script(c) == Script::Katakana);
    let hiragana =
        |character: Option<&char>| character.is_some_and(|&c| script(c) == Script::Hiragana);
    characters.iter().enumerate().all(|(index, character)| {
        let previous = index
            .checked_sub(1)
            .and_then(|before| characters.get(before));
        let next = characters.get(index + 1);
        match character {
            '\u{30FC}' => previous.is_some_and(|&c| is_kana(c)),
            '\u{4E00}' | '\u{4E8C}' => !katakana(previous) && !katakana(next),
            '\u{30FB}' => katakana(previous) && katakana(next),
            'ヘ' | 'ベ' | 'ペ' => !hiragana(previous) && !hiragana(next),
            'へ' | 'べ' | 'ぺ' => !katakana(previous) && !katakana(next),
            _ => true,
        }
    })
}
