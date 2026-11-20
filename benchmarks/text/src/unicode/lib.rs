use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use icu_segmenter::options::{LineBreakOptions, WordBreakInvariantOptions};
use icu_segmenter::{GraphemeClusterSegmenter, LineSegmenter, WordSegmenter};
use rikka_text_unicode::{BaseDirection, Bidi};
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

use text_samples::{PARAGRAPH_CHARACTERS, paragraph_cases, repeated};

const THAI: &str = "ภาษาไทยเป็นภาษาที่มีระดับเสียงของคำแน่นอนหรือวรรณยุกต์เช่นเดียวกับภาษาจีน ";
const HINDI: &str = "हिन्दी विश्व की एक प्रमुख भाषा है और भारत की राजभाषा है। क्षत्रिय श्री द्वार ";
const EMOJI: &str = "Family 👨‍👩‍👧‍👦 flags 🇯🇵🇫🇷 skin 👍🏽 keycap 1️⃣ zwj 🏳️‍🌈 text ";
const DECOMPOSED: &str = "Tie\u{302}\u{301}ng Vie\u{323}\u{302}t ca\u{301}c ha\u{300}ng 한국어 \u{1100}\u{1161}\u{11A8} Å ﬁ ";

#[derive(Clone, Debug)]
pub struct UnicodeCase {
    pub name: &'static str,
    pub text: String,
}

pub fn unicode_cases() -> Vec<UnicodeCase> {
    let mut cases: Vec<UnicodeCase> = paragraph_cases()
        .into_iter()
        .filter(|case| case.text.chars().count() >= PARAGRAPH_CHARACTERS)
        .map(|case| UnicodeCase {
            name: case.name,
            text: case.text,
        })
        .collect();
    for (name, sentence) in [
        ("thai", THAI),
        ("hindi", HINDI),
        ("emoji", EMOJI),
        ("decomposed", DECOMPOSED),
    ] {
        cases.push(UnicodeCase {
            name,
            text: repeated(sentence),
        });
    }
    cases
}

pub fn unicode_case(name: &str) -> Option<UnicodeCase> {
    unicode_cases().into_iter().find(|case| case.name == name)
}

fn bounded(text: &str, mut starts: Vec<usize>) -> Vec<usize> {
    starts.push(0);
    starts.push(text.len());
    starts.sort_unstable();
    starts.dedup();
    starts
}

pub fn rikka_graphemes(text: &str) -> Vec<usize> {
    bounded(
        text,
        rikka_text_unicode::graphemes(text)
            .map(|(start, _)| start)
            .collect(),
    )
}

pub fn icu_graphemes(text: &str) -> Vec<usize> {
    bounded(
        text,
        GraphemeClusterSegmenter::new().segment_str(text).collect(),
    )
}

pub fn unicode_rs_graphemes(text: &str) -> Vec<usize> {
    bounded(
        text,
        text.grapheme_indices(true)
            .map(|(start, _)| start)
            .collect(),
    )
}

pub fn rikka_words(text: &str) -> Vec<usize> {
    bounded(
        text,
        rikka_text_unicode::word_segments(text)
            .map(|segment| segment.start)
            .collect(),
    )
}

pub fn icu_words(text: &str) -> Vec<usize> {
    bounded(
        text,
        WordSegmenter::new_dictionary(WordBreakInvariantOptions::default())
            .segment_str(text)
            .collect(),
    )
}

pub fn unicode_rs_words(text: &str) -> Vec<usize> {
    bounded(
        text,
        text.split_word_bound_indices()
            .map(|(start, _)| start)
            .collect(),
    )
}

pub fn rikka_lines(text: &str) -> Vec<usize> {
    bounded(
        text,
        rikka_text_unicode::line_breaks(text)
            .map(|line_break| line_break.offset)
            .collect(),
    )
}

pub fn icu_lines(text: &str) -> Vec<usize> {
    bounded(
        text,
        LineSegmenter::new_dictionary(LineBreakOptions::default())
            .segment_str(text)
            .collect(),
    )
}

pub fn unicode_rs_lines(text: &str) -> Vec<usize> {
    bounded(
        text,
        unicode_linebreak::linebreaks(text)
            .map(|(offset, _)| offset)
            .collect(),
    )
}

pub fn rikka_nfc(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    rikka_text_unicode::nfc_into(text, &mut output);
    output
}

pub fn rikka_nfd(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    rikka_text_unicode::nfd_into(text, &mut output);
    output
}

pub fn icu_nfc(text: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(text)
        .into_owned()
}

pub fn icu_nfd(text: &str) -> String {
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(text)
        .into_owned()
}

pub fn unicode_rs_nfc(text: &str) -> String {
    text.nfc().collect()
}

pub fn unicode_rs_nfd(text: &str) -> String {
    text.nfd().collect()
}

pub fn rikka_bidi(bidi: &mut Bidi, text: &str) -> usize {
    bidi.analyze(text, BaseDirection::Auto);
    bidi.levels().len()
}

pub fn unicode_rs_bidi(text: &str) -> usize {
    unicode_bidi::BidiInfo::new(text, None).levels.len()
}
