use rikka_text_unicode::{LineBreaks, WordSegments, graphemes, word_segments};

use crate::support::breaks::assert_breaks;

#[test]
fn grapheme_break_conformance() {
    assert_breaks(
        "GraphemeBreakTest",
        include_str!("../ucd/auxiliary/GraphemeBreakTest.txt"),
        |text| {
            graphemes(text)
                .map(|(start, cluster)| start + cluster.len())
                .collect()
        },
    );
}

#[test]
fn line_break_conformance() {
    assert_breaks(
        "LineBreakTest",
        include_str!("../ucd/auxiliary/LineBreakTest.txt"),
        |text| {
            LineBreaks::untailored(text)
                .map(|line_break| line_break.offset)
                .collect()
        },
    );
}

#[test]
fn word_break_conformance() {
    assert_breaks(
        "WordBreakTest",
        include_str!("../ucd/auxiliary/WordBreakTest.txt"),
        |text| {
            WordSegments::untailored(text)
                .map(|segment| segment.start + segment.text.len())
                .collect()
        },
    );
}

#[test]
fn word_segments_cover_text() {
    let text = "Hello, \u{4E16}\u{754C}! e.g. \u{05D0}\"\u{05D1}";
    let mut end = 0;
    for segment in word_segments(text) {
        assert_eq!(segment.start, end);
        end += segment.text.len();
    }
    assert_eq!(end, text.len());
}
