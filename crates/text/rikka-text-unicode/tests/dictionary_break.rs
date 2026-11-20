use crate::support::icu_breaks::{IcuCase, Iteration, icu_cases};
use rikka_text_unicode::{
    LineBreakClass, Script, dictionary_breaks, line_break_class, line_breaks, script, word_segments,
};

const ICU_BREAK_TESTS: &str = include_str!("icu/rbbitst.txt");

fn has_dictionary_script(text: &str) -> bool {
    text.chars().any(|character| {
        line_break_class(character) == LineBreakClass::ComplexContext
            && matches!(
                script(character),
                Script::Thai | Script::Lao | Script::Khmer | Script::Myanmar
            )
    })
}

fn has_ideographs(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            script(character),
            Script::Han | Script::Hiragana | Script::Katakana
        )
    })
}

fn line_offsets(text: &str) -> Vec<usize> {
    line_breaks(text)
        .map(|line_break| line_break.offset)
        .collect()
}

fn word_ends(text: &str) -> Vec<usize> {
    word_segments(text)
        .map(|segment| segment.start + segment.text.len())
        .collect()
}

fn assert_icu_cases(
    iteration: Iteration,
    covers: fn(&str) -> bool,
    least: usize,
    segment: impl Fn(&str) -> Vec<usize>,
) {
    let cases: Vec<IcuCase> = icu_cases(ICU_BREAK_TESTS)
        .into_iter()
        .filter(|case| case.iteration == iteration && covers(&case.text))
        .collect();
    assert!(
        cases.len() >= least,
        "only {} cases were found",
        cases.len()
    );
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let actual = segment(&case.text);
            (actual != case.breaks).then(|| {
                format!(
                    "rbbitst.txt:{}: {:?}\n  expected {:?}\n  got      {:?}",
                    case.line, case.text, case.breaks, actual
                )
            })
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} ICU cases fail:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn dictionary_line_breaks_match_icu() {
    assert_icu_cases(Iteration::Line, has_dictionary_script, 20, line_offsets);
}

#[test]
fn dictionary_word_segments_match_icu() {
    assert_icu_cases(Iteration::Word, has_dictionary_script, 20, word_ends);
}

#[test]
fn ideographic_word_segments_match_icu() {
    assert_icu_cases(Iteration::Word, has_ideographs, 40, word_ends);
}

#[test]
fn breaks_keep_marks_with_base() {
    let text = "น้ำใจไมตรีที่ดีงามของคนไทยทั่วประเทศ";
    for offset in dictionary_breaks(text) {
        let following = text[offset..].chars().next().unwrap();
        assert!(
            !rikka_text_unicode::general_category(following).is_mark(),
            "a break at {offset} precedes a mark"
        );
    }
    assert!(dictionary_breaks(text).count() >= 5);
}

#[test]
fn long_sa_runs_segment_without_gaps() {
    let text = "ประเทศไทยมีประชากรประมาณหกสิบหกล้านคน".repeat(200);
    let mut end = 0;
    for segment in word_segments(&text) {
        assert_eq!(segment.start, end);
        assert!(segment.is_word);
        end += segment.text.len();
    }
    assert_eq!(end, text.len());
    assert!(line_breaks(&text).count() > 1000);
}
