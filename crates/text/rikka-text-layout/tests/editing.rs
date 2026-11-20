use crate::support::{assert_close, layout_text, params, wrapped};
use rikka_text_layout::{
    Affinity, Cursor, Movement, ParagraphLayout, Wrap, next_grapheme_boundary,
    previous_grapheme_boundary,
};
use rikka_text_layout::{
    TextSource, WordRule, next_word_boundary, previous_word_boundary, word_range_at,
};
use std::borrow::Cow;
use std::ops::Range;

const MIXED: &str = "abc \u{5D0}\u{5D1}\u{5D2} def";
const HEBREW: &str = "\u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{5E2}\u{5D5}\u{5DC}\u{5DD}";
const HEBREW_WITH_LATIN: &str = "\u{5D0}\u{5D1}\u{5D2} abc \u{5D3}\u{5D4}\u{5D5}";
const HEBREW_WITH_NUMBER: &str = "\u{5E9}\u{5E0}\u{5EA} 2024 \u{5D8}\u{5D5}\u{5D1}\u{5D4}";

fn boundaries(text: &str) -> Vec<usize> {
    let mut boundaries: Vec<usize> = text_unicode_boundaries(text);
    boundaries.push(text.len());
    boundaries
}

fn text_unicode_boundaries(text: &str) -> Vec<usize> {
    let mut at = 0;
    let mut starts = Vec::new();
    while at < text.len() {
        starts.push(at);
        at = next_grapheme_boundary(text, at);
    }
    starts
}

fn level_at(layout: &ParagraphLayout, byte: usize) -> bool {
    layout
        .all_glyphs()
        .iter()
        .find(|glyph| glyph.cluster().contains(&byte))
        .is_some_and(|glyph| glyph.is_rtl())
}

fn assert_round_trips(layout: &ParagraphLayout, nudged: bool) {
    let text = layout.text();
    for index in boundaries(text) {
        for affinity in [Affinity::Before, Affinity::After] {
            if (affinity == Affinity::Before && index == 0)
                || (affinity == Affinity::After && index == text.len())
            {
                continue;
            }
            let cursor = Cursor::new(index, affinity);
            let caret = layout.caret(cursor);
            let x = match nudged {
                false => caret.x,
                true => {
                    let attached = match affinity {
                        Affinity::After => index,
                        Affinity::Before => previous_grapheme_boundary(text, index),
                    };
                    let rtl = level_at(layout, attached);
                    let into_left_edge = (affinity == Affinity::After) != rtl;
                    caret.x + if into_left_edge { 0.25 } else { -0.25 }
                }
            };
            let hit = layout.hit(x, caret.top + caret.height / 2.0);
            assert_eq!(
                hit.index, index,
                "{text:?}: {cursor:?} has its caret at {x}, which hits {hit:?}"
            );
        }
    }
}

#[test]
fn rtl_caret_hits_round_trip() {
    assert_round_trips(&layout_text(HEBREW, &params()), false);
    assert_round_trips(&layout_text(HEBREW, &wrapped(40.0, Wrap::Word)), false);
    let arabic =
        "\u{645}\u{631}\u{62D}\u{628}\u{627} \u{628}\u{627}\u{644}\u{639}\u{627}\u{644}\u{645}";
    assert_round_trips(&layout_text(arabic, &params()), true);
}

#[test]
fn mixed_text_hits_round_trip() {
    for text in [MIXED, HEBREW_WITH_LATIN, HEBREW_WITH_NUMBER] {
        assert_round_trips(&layout_text(text, &params()), true);
    }
}

#[test]
fn selection_gives_one_rect_per_row() {
    let layout = layout_text("first second third", &wrapped(60.0, Wrap::Word));
    let mut rects = Vec::new();
    layout.selection_rects(2..15, &mut rects);
    assert_eq!(rects.len(), layout.rows().len());
    for (rect, row) in rects.iter().zip(layout.rows()) {
        assert_close(rect.y, row.top(), "row top");
        assert_close(rect.height, row.height(), "row height");
    }
}

#[test]
fn visual_steps_visit_every_position() {
    for text in [MIXED, HEBREW_WITH_LATIN, HEBREW_WITH_NUMBER] {
        let layout = layout_text(text, &params());
        let row = &layout.rows()[0];
        let leftmost = layout.hit(row.left() - 10.0, 1.0);
        assert_close(
            layout.caret(leftmost).x,
            row.left(),
            "starts at the left edge",
        );
        let mut cursor = leftmost;
        let mut xs = vec![layout.caret(cursor).x];
        while let Some(next) = layout.visual_step(cursor, Movement::Right) {
            cursor = next;
            xs.push(layout.caret(cursor).x);
        }
        let graphemes = boundaries(text).len() - 1;
        assert_eq!(xs.len(), graphemes + 1, "{text:?}: {xs:?}");
        assert!(
            xs.windows(2).all(|pair| pair[0] < pair[1]),
            "{text:?}: {xs:?}"
        );
        assert_close(*xs.last().unwrap(), row.right(), "ends at the right edge");
        let mut steps = 0;
        while let Some(previous) = layout.visual_step(cursor, Movement::Left) {
            cursor = previous;
            steps += 1;
        }
        assert_eq!(steps, graphemes);
        assert_close(layout.caret(cursor).x, row.left(), "back at the left edge");
    }
}

const MANY_SCRIPTS: &str = "e\u{301}x \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} สวัสดีครับ 한국어 \u{1F1EF}\u{1F1F5}\u{1F1EF} a_b, c++; ไปที่บ้านของฉัน\tक्षि end. 日本語のテキストを書く我们在学习中文 ";

struct Whole<'a>(&'a str);

impl TextSource for Whole<'_> {
    fn byte_len(&self) -> usize {
        self.0.len()
    }

    fn covering(&self, _: Range<usize>) -> (usize, Cow<'_, str>) {
        (0, Cow::Borrowed(self.0))
    }
}

#[test]
fn windowed_boundaries_match_whole_text() {
    let text = MANY_SCRIPTS.repeat(24);
    let whole = Whole(&text);
    for (index, _) in text.char_indices() {
        assert_eq!(
            next_word_boundary(text.as_str(), index, WordRule::Prose),
            next_word_boundary(&whole, index, WordRule::Prose),
            "next word from {index}"
        );
        assert_eq!(
            previous_word_boundary(text.as_str(), index, WordRule::Prose),
            previous_word_boundary(&whole, index, WordRule::Prose),
            "previous word from {index}"
        );
        for affinity in [Affinity::After, Affinity::Before] {
            assert_eq!(
                word_range_at(text.as_str(), index, affinity, WordRule::Prose),
                word_range_at(&whole, index, affinity, WordRule::Prose),
                "word around {index} {affinity:?}"
            );
        }
        assert_eq!(
            next_grapheme_boundary(text.as_str(), index),
            next_grapheme_boundary(&whole, index),
            "next grapheme from {index}"
        );
        assert_eq!(
            previous_grapheme_boundary(text.as_str(), index),
            previous_grapheme_boundary(&whole, index),
            "previous grapheme from {index}"
        );
    }
}

#[test]
fn long_dictionary_runs_step_like_the_whole_run() {
    for sample in ["我们在学习中文", "日本語のテキストを書く", "ไปที่บ้านของฉัน"]
    {
        let text = sample.repeat(40_000 / sample.len());
        let whole = Whole(&text);
        let middle = text.len() / 2;
        let middle = middle - middle % sample.len();
        for (offset, _) in sample.char_indices() {
            let index = middle + offset;
            for rule in [WordRule::Prose, WordRule::Code] {
                assert_eq!(
                    next_word_boundary(text.as_str(), index, rule),
                    next_word_boundary(&whole, index, rule),
                    "{sample}: next word from {index}"
                );
                assert_eq!(
                    previous_word_boundary(text.as_str(), index, rule),
                    previous_word_boundary(&whole, index, rule),
                    "{sample}: previous word from {index}"
                );
                for affinity in [Affinity::After, Affinity::Before] {
                    assert_eq!(
                        word_range_at(text.as_str(), index, affinity, rule),
                        word_range_at(&whole, index, affinity, rule),
                        "{sample}: word around {index} {affinity:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn index_inside_a_character_answers_for_its_start() {
    let text = "\u{E9}".repeat(300);
    let text = text.as_str();
    for inside in [1, 301, 599] {
        let start = inside - 1;
        assert_eq!(
            next_grapheme_boundary(text, inside),
            next_grapheme_boundary(text, start)
        );
        assert_eq!(
            previous_grapheme_boundary(text, inside),
            previous_grapheme_boundary(text, start)
        );
        assert_eq!(
            next_word_boundary(text, inside, WordRule::Prose),
            next_word_boundary(text, start, WordRule::Prose)
        );
        assert_eq!(
            previous_word_boundary(text, inside, WordRule::Prose),
            previous_word_boundary(text, start, WordRule::Prose)
        );
        assert_eq!(
            word_range_at(text, inside, Affinity::After, WordRule::Prose),
            word_range_at(text, start, Affinity::After, WordRule::Prose)
        );
    }
}

fn marked(stops: &str) -> (String, Vec<usize>) {
    let mut text = String::new();
    let mut marks = Vec::new();
    for piece in stops.split_inclusive('|') {
        match piece.strip_suffix('|') {
            Some(piece) => {
                text.push_str(piece);
                marks.push(text.len());
            }
            None => text.push_str(piece),
        }
    }
    (text, marks)
}

#[test]
fn word_steps_follow_their_rule() {
    let table = [
        (WordRule::Prose, "can't |stop|", "|can't |stop"),
        (
            WordRule::Prose,
            "foo.bar|(|x|) |-> |y|",
            "|foo.bar|(|x|) |-> |y",
        ),
        (WordRule::Prose, "f|(|1,2|)|", "|f|(|1,2|)"),
        (
            WordRule::Prose,
            "日本語|の|テキスト|を|書く|",
            "|日本語|の|テキスト|を|書く",
        ),
        (
            WordRule::Prose,
            "ラーメン|、|うまい|",
            "|ラーメン|、|うまい",
        ),
        (
            WordRule::Prose,
            "我们|在|学习|中文|。|",
            "|我们|在|学习|中文|。",
        ),
        (WordRule::Prose, "שלום |עולם|", "|שלום |עולם"),
        (WordRule::Code, "名前|.|サイズ|", "|名前|.|サイズ"),
        (WordRule::Code, "can|'|t |stop|", "|can|'|t |stop"),
        (
            WordRule::Code,
            "foo|.|bar|(|x|) |-> |y|",
            "|foo|.|bar|(|x|) |-> |y",
        ),
        (WordRule::Code, "a|::|b|", "|a|::|b"),
        (WordRule::Code, "f|(|1|,|2|)|", "|f|(|1|,|2|)"),
        (
            WordRule::Code,
            "snake_case |+ |camelCase2|",
            "|snake_case |+ |camelCase2",
        ),
        (WordRule::Code, "x |= |1|.|5e3|", "|x |= |1|.|5e3"),
    ];
    for (rule, forward, backward) in table {
        let (text, expected) = marked(forward);
        let mut stops = Vec::new();
        let mut at = 0;
        while at < text.len() {
            at = next_word_boundary(text.as_str(), at, rule);
            stops.push(at);
        }
        assert_eq!(stops, expected, "{rule:?} {text:?} forward");
        let (_, mut expected) = marked(backward);
        expected.reverse();
        let mut stops = Vec::new();
        while at > 0 {
            at = previous_word_boundary(text.as_str(), at, rule);
            stops.push(at);
        }
        assert_eq!(stops, expected, "{rule:?} {text:?} backward");
    }
}

#[test]
fn word_at_double_click_covers_its_run() {
    for (rule, text, inside, affinity, word) in [
        (
            WordRule::Prose,
            "日本語のテキストを書く",
            "日本語のテ".len(),
            Affinity::After,
            "テキスト",
        ),
        (
            WordRule::Prose,
            "ไปที่บ้านของฉัน",
            "ไปที่บ้".len(),
            Affinity::After,
            "บ้าน",
        ),
        (WordRule::Prose, "a == b", 3, Affinity::After, "=="),
        (WordRule::Prose, "foo.bar", 5, Affinity::After, "foo.bar"),
        (WordRule::Code, "foo.bar", 5, Affinity::After, "bar"),
        (
            WordRule::Code,
            "snake_case word",
            2,
            Affinity::After,
            "snake_case",
        ),
        (WordRule::Prose, "foo bar", 3, Affinity::Before, "foo"),
        (WordRule::Prose, "foo bar", 3, Affinity::After, " "),
        (WordRule::Prose, "foo bar", 4, Affinity::Before, " "),
        (WordRule::Prose, "foo bar", 4, Affinity::After, "bar"),
        (WordRule::Prose, "foo bar", 0, Affinity::Before, "foo"),
        (WordRule::Prose, "foo bar", 7, Affinity::After, "bar"),
        (WordRule::Code, "foo.bar", 3, Affinity::Before, "foo"),
    ] {
        let range = word_range_at(text, inside, affinity, rule);
        assert_eq!(
            &text[range], word,
            "{rule:?} {text:?} {inside} {affinity:?}"
        );
    }
}

#[test]
fn row_edges_stay_on_their_row() {
    let layout = layout_text("first second third", &wrapped(60.0, Wrap::Word));
    assert!(layout.rows().len() > 1);
    for (index, row) in layout.rows().iter().enumerate() {
        let inside = Cursor::new(row.text_range().start + 1, Affinity::After);
        let start = layout.row_start(inside);
        let end = layout.row_end(inside);
        assert_eq!(start.index, row.text_range().start);
        assert_eq!(end.index, row.text_range().end);
        assert_eq!(layout.caret(start).row, index);
        assert_eq!(layout.caret(end).row, index);
    }
}
