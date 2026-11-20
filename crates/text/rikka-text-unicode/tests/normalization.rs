use std::collections::HashSet;

use crate::support::codepoints::parse_codepoints;
use rikka_text_unicode::{canonical_decomposition, is_nfc, is_nfd, nfc, nfd, nfkc, nfkd};

#[test]
fn normalization_conformance() {
    let mut listed = HashSet::new();
    let mut part = "";
    let mut failures = Vec::new();
    for (index, row) in include_str!("../ucd/NormalizationTest.txt")
        .lines()
        .enumerate()
    {
        let row = row.split('#').next().unwrap_or("").trim();
        if row.starts_with('@') {
            part = row;
            continue;
        }
        let fields: Vec<String> = row.split(';').take(5).map(parse_codepoints).collect();
        let [
            source,
            composed,
            decomposed,
            compatibility_composed,
            compatibility_decomposed,
        ] = &fields[..]
        else {
            continue;
        };
        if part == "@Part1" {
            listed.extend(source.chars());
        }
        let checks = [
            (nfc(source), composed),
            (nfc(composed), composed),
            (nfc(decomposed), composed),
            (nfc(compatibility_composed), compatibility_composed),
            (nfc(compatibility_decomposed), compatibility_composed),
            (nfd(source), decomposed),
            (nfd(composed), decomposed),
            (nfd(decomposed), decomposed),
            (nfd(compatibility_composed), compatibility_decomposed),
            (nfd(compatibility_decomposed), compatibility_decomposed),
            (nfkc(source), compatibility_composed),
            (nfkc(composed), compatibility_composed),
            (nfkc(decomposed), compatibility_composed),
            (nfkc(compatibility_composed), compatibility_composed),
            (nfkc(compatibility_decomposed), compatibility_composed),
            (nfkd(source), compatibility_decomposed),
            (nfkd(composed), compatibility_decomposed),
            (nfkd(decomposed), compatibility_decomposed),
            (nfkd(compatibility_composed), compatibility_decomposed),
            (nfkd(compatibility_decomposed), compatibility_decomposed),
        ];
        for (actual, expected) in checks {
            if actual != expected.as_str() {
                failures.push(format!(
                    "line {}: expected {expected:?}, got {actual:?}",
                    index + 1
                ));
            }
        }
        if !is_nfc(composed) || !is_nfd(decomposed) {
            failures.push(format!(
                "line {}: quick checks reject normalized forms",
                index + 1
            ));
        }
        if is_nfc(source) != (source == composed) || is_nfd(source) != (source == decomposed) {
            failures.push(format!(
                "line {}: quick checks disagree with normalization",
                index + 1
            ));
        }
    }
    let mut buffer = [0; 4];
    for character in (0..=0x10FFFF).filter_map(char::from_u32) {
        if listed.contains(&character) {
            continue;
        }
        let text = character.encode_utf8(&mut buffer);
        if [nfc(text), nfd(text), nfkc(text), nfkd(text)]
            .iter()
            .any(|normalized| normalized != text)
        {
            failures.push(format!(
                "U+{:04X} changes under normalization",
                character as u32
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

#[test]
fn canonical_decompositions_agree_with_nfd() {
    for character in (0..=0x10FFFF).filter_map(char::from_u32) {
        let parts: String = canonical_decomposition(character).collect();
        assert_eq!(
            parts,
            nfd(&character.to_string()),
            "U+{:04X}",
            character as u32
        );
    }
    let parts: Vec<char> = canonical_decomposition('\u{1F82}').collect();
    assert_eq!(parts, ['\u{03B1}', '\u{0313}', '\u{0300}', '\u{0345}']);
}

#[test]
fn long_mark_runs_reorder_stably() {
    let above = "\u{301}\u{300}".repeat(40);
    let below = "\u{316}\u{317}".repeat(40);
    let text = format!("a{above}{below}");
    assert_eq!(nfd(&text), format!("a{below}{above}"));
}
