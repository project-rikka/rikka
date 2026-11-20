use rikka_text_unicode::{general_category, nfkd};

const QUOTES: &[char] = &[
    '\u{00ab}', '\u{00bb}', '\u{201e}', '\u{201c}', '\u{201f}', '\u{201d}', '\u{275d}', '\u{275e}',
    '\u{276e}', '\u{276f}', '\u{2e42}', '\u{301d}', '\u{301e}', '\u{301f}', '\u{ff02}',
];
const APOSTROPHES: &[char] = &['\u{2018}', '\u{2019}', '\u{02bc}'];

pub(crate) fn normalize(text: &str) -> String {
    nfkd(&text.to_lowercase())
        .chars()
        .filter(|character| !general_category(*character).is_mark())
        .map(|character| match APOSTROPHES.contains(&character) {
            true => '\'',
            false => character,
        })
        .collect()
}

pub(crate) fn keywords(query: &str) -> Vec<String> {
    let text: String = normalize(query)
        .chars()
        .map(|character| match QUOTES.contains(&character) {
            true => '"',
            false => character,
        })
        .collect();

    let segments: Vec<&str> = text.split('"').collect();
    let last = segments.len() - 1;
    let mut keywords = Vec::new();

    for (index, segment) in segments.into_iter().enumerate() {
        let quoted = index % 2 == 1 && index != last;
        if quoted {
            if !segment.is_empty() {
                keywords.push(segment.to_owned());
            }
        } else {
            keywords.extend(segment.split_whitespace().map(str::to_owned));
        }
    }
    keywords
}
