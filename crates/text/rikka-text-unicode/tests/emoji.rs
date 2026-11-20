use crate::support::codepoints::parse_codepoints;
use rikka_text_unicode::{
    EmojiPresentation, basic_emoji, emoji_presentation, emoji_sequence, graphemes,
};

#[test]
fn emoji_test_sequences_are_single_graphemes() {
    let mut failures = Vec::new();
    let mut cases = 0;
    for (index, row) in include_str!("../ucd/emoji/emoji-test.txt")
        .lines()
        .enumerate()
    {
        let row = row.split('#').next().unwrap_or("").trim();
        let Some((codepoints, status)) = row.split_once(';') else {
            continue;
        };
        let text = parse_codepoints(codepoints);
        let status = status.trim();
        cases += 1;
        let sequence = emoji_sequence(&text);
        if sequence.map(|sequence| sequence.len) != Some(text.len()) {
            failures.push(format!(
                "line {}: {codepoints} parsed as {sequence:?}",
                index + 1
            ));
        }
        if graphemes(&text).count() != 1 {
            failures.push(format!("line {}: {codepoints} spans graphemes", index + 1));
        }
        let presentation = emoji_presentation(&text);
        if status == "fully-qualified" && presentation != Some(EmojiPresentation::Emoji) {
            failures.push(format!(
                "line {}: {codepoints} presents as {presentation:?}",
                index + 1
            ));
        }
    }
    assert!(cases > 0);
    assert!(
        failures.is_empty(),
        "{} of {cases} sequences fail:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

#[test]
fn variation_selectors_choose_presentation() {
    assert_eq!(
        emoji_presentation("\u{2764}"),
        Some(EmojiPresentation::Text)
    );
    assert_eq!(
        emoji_presentation("\u{2764}\u{FE0F}"),
        Some(EmojiPresentation::Emoji)
    );
    assert_eq!(
        emoji_presentation("\u{1F600}"),
        Some(EmojiPresentation::Emoji)
    );
    assert_eq!(
        emoji_presentation("\u{1F600}\u{FE0E}"),
        Some(EmojiPresentation::Text)
    );
    assert_eq!(emoji_presentation("1"), Some(EmojiPresentation::Text));
    assert_eq!(
        emoji_presentation("1\u{20E3}"),
        Some(EmojiPresentation::Emoji)
    );
    assert_eq!(emoji_presentation("x"), None);
}

#[test]
fn basic_emoji_follow_emoji_test() {
    let mut failures = Vec::new();
    for (index, row) in include_str!("../ucd/emoji/emoji-test.txt")
        .lines()
        .enumerate()
    {
        let row = row.split('#').next().unwrap_or("").trim();
        let Some((codepoints, status)) = row.split_once(';') else {
            continue;
        };
        let text = parse_codepoints(codepoints);
        let mut characters = text.chars().skip(1);
        if !matches!(
            (characters.next(), characters.next()),
            (None, _) | (Some('\u{FE0F}'), None)
        ) {
            continue;
        }
        let expected =
            matches!(status.trim(), "fully-qualified" | "component").then_some(text.len());
        let found = basic_emoji(&text).map(|sequence| sequence.len);
        if found != expected {
            failures.push(format!("line {}: {codepoints} found {found:?}", index + 1));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn components_are_not_basic_emoji() {
    let cases: &[(&str, Option<usize>)] = &[
        ("1", None),
        ("1\u{FE0F}", None),
        ("#\u{FE0F}\u{20E3}", None),
        ("\u{1F1E6}", None),
        ("\u{1F1E6}\u{FE0F}", None),
        ("\u{2614}\u{FE0E}", None),
        ("\u{2614}", Some(3)),
        ("\u{1F3FB}", Some(4)),
        ("\u{1F44B}\u{1F3FD}", Some(4)),
        ("\u{00A9}\u{FE0F}", Some(5)),
        ("x", None),
    ];
    for (text, expected) in cases {
        assert_eq!(
            basic_emoji(text).map(|sequence| sequence.len),
            *expected,
            "{text:?}"
        );
    }
}
