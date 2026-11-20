use rikka_text_document::{Content, Erase, Granularity, Travel};
use rikka_text_layout::{
    Affinity, Cursor, LayoutParams, Paragraph, ParagraphLayout, TextStyle, WordRule, Wrap,
    next_grapheme_boundary,
};

const LTR_WITH_HEBREW: &str = "abc \u{5D0}\u{5D1}\u{5D2} def";
const HEBREW_WITH_LATIN: &str = "\u{5D0}\u{5D1}\u{5D2} abc \u{5D3}\u{5D4}\u{5D5}";
const ARABIC_WITH_NUMBER: &str = "\u{645}\u{631}\u{62D}\u{628}\u{627} 2024 \u{628}\u{627}\u{644}";

fn laid(text: &str, width: Option<f32>) -> ParagraphLayout {
    let params = LayoutParams {
        width,
        wrap: Wrap::Word,
        ..LayoutParams::new(16.0, 22.0)
    };
    Paragraph::new(text, &TextStyle::new(["Inter"]))
        .shape()
        .layout(&params)
}

fn content_at(text: &str, at: usize) -> Content {
    let mut content = Content::new(text);
    content.place_caret(at, false);
    content
}

fn caret_x(content: &Content, layout: &ParagraphLayout) -> f32 {
    layout
        .caret(Cursor::new(content.caret(), content.affinity()))
        .x
}

fn graphemes(text: &str) -> usize {
    let mut count = 0;
    let mut at = 0;
    while at < text.len() {
        at = next_grapheme_boundary(text, at);
        count += 1;
    }
    count
}

#[test]
fn arrows_move_in_visual_order() {
    for text in [LTR_WITH_HEBREW, HEBREW_WITH_LATIN, ARABIC_WITH_NUMBER] {
        let layout = laid(text, None);
        let row = &layout.rows()[0];
        let leftmost = layout.hit(row.left() - 10.0, 1.0);
        let mut content = Content::new(text);
        content.place_cursor(leftmost.index, leftmost.affinity, false);
        let mut xs = vec![caret_x(&content, &layout)];
        while content.travel(Travel::Right, false, Some(&layout), WordRule::Prose) {
            xs.push(caret_x(&content, &layout));
        }
        assert_eq!(xs.len(), graphemes(text) + 1, "{text:?}: {xs:?}");
        assert!(
            xs.windows(2).all(|pair| pair[0] < pair[1]),
            "{text:?} moves right: {xs:?}"
        );
        let mut back = vec![caret_x(&content, &layout)];
        while content.travel(Travel::Left, false, Some(&layout), WordRule::Prose) {
            back.push(caret_x(&content, &layout));
        }
        back.reverse();
        assert_eq!(back, xs, "{text:?} retraces its steps");
    }
}

#[test]
fn extending_through_mixed_text_selects_a_logical_range() {
    let layout = laid(LTR_WITH_HEBREW, None);
    let mut content = Content::new(LTR_WITH_HEBREW);
    content.place_cursor("abc ".len(), Affinity::Before, false);
    assert!(content.travel(Travel::Right, true, Some(&layout), WordRule::Prose));
    assert!(content.travel(Travel::Right, true, Some(&layout), WordRule::Prose));
    assert_eq!(content.selection(), "abc ".len().."abc \u{5D0}".len());
    assert_eq!(content.selected_text(), "\u{5D0}");
    assert!(content.travel(Travel::Right, false, Some(&layout), WordRule::Prose));
    assert!(!content.has_selection());
    assert_eq!(content.caret(), "abc \u{5D0}".len());
}

#[test]
fn arrows_cross_lines_by_paragraph_direction() {
    let text = "abc\n\u{5E9}\u{5DC}\u{5D5}\u{5DD}";
    let first = laid("abc", None);
    let second = laid(&text[4..], None);
    let mut content = content_at(text, 3);
    assert!(content.travel(Travel::Right, false, Some(&first), WordRule::Prose));
    assert_eq!(
        content.caret(),
        4,
        "right edge of a left-to-right line leads on"
    );
    assert!(content.travel(Travel::Right, false, Some(&second), WordRule::Prose));
    assert_eq!(
        content.caret(),
        3,
        "right edge of a right-to-left line leads back"
    );
}

#[test]
fn home_and_end_stay_on_the_shown_row() {
    let text = "first second third";
    let layout = laid(text, Some(60.0));
    let rows = layout.rows();
    assert!(rows.len() > 2);
    let middle = rows[1].text_range();
    let mut content = content_at(text, middle.start + 2);
    assert!(content.travel(Travel::LineEnd, false, Some(&layout), WordRule::Prose));
    assert_eq!(content.caret(), middle.end);
    assert_eq!(
        layout
            .caret(Cursor::new(content.caret(), content.affinity()))
            .row,
        1
    );
    assert!(content.travel(Travel::LineStart, false, Some(&layout), WordRule::Prose));
    assert_eq!(content.caret(), middle.start);
    assert_eq!(
        layout
            .caret(Cursor::new(content.caret(), content.affinity()))
            .row,
        1
    );
}

#[test]
fn word_arrows_follow_paragraph_direction() {
    let text = "\u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{5E2}\u{5D5}\u{5DC}\u{5DD}";
    let layout = laid(text, None);
    let mut content = content_at(text, 0);
    assert!(content.travel(Travel::WordLeft, false, Some(&layout), WordRule::Prose));
    assert_eq!(content.caret(), "\u{5E9}\u{5DC}\u{5D5}\u{5DD} ".len());
    assert!(content.travel(Travel::WordRight, false, Some(&layout), WordRule::Prose));
    assert_eq!(content.caret(), 0);
}

#[test]
fn word_steps_use_dictionary_and_ideographs() {
    for (text, stops) in [
        (
            "สวัสดีครับ สบายดี",
            &["สวัสดี", "สวัสดีครับ ", "สวัสดีครับ สบาย", "สวัสดีครับ สบายดี"][..],
        ),
        (
            "日本語のテキストを書く",
            &[
                "日本語",
                "日本語の",
                "日本語のテキスト",
                "日本語のテキストを",
                "日本語のテキストを書く",
            ][..],
        ),
    ] {
        let mut content = content_at(text, 0);
        let mut reached = Vec::new();
        while content.travel(Travel::WordRight, false, None, WordRule::Prose) {
            reached.push(content.caret());
        }
        let expected: Vec<usize> = stops.iter().map(|stop| stop.len()).collect();
        assert_eq!(reached, expected, "{text:?}");
    }

    let mut content = content_at("สวัสดีครับ สบายดี", "สวัสดีครับ สบายดี".len());
    content.travel(Travel::WordLeft, false, None, WordRule::Prose);
    assert_eq!(content.caret(), "สวัสดีครับ สบาย".len());

    let content = Content::new("ไปที่บ้านของฉัน\nok");
    assert_eq!(word_at(&content, "ไปที่บ้".len()), "บ้าน");

    let content = Content::new("日本語のテキストを書く");
    assert_eq!(word_at(&content, "日本語のテ".len()), "テキスト");
    let mut content = content_at("日本語のテキストを書く", "日本語のテキストを".len());
    assert!(content.erase(Erase::WordBack, WordRule::Prose));
    assert_eq!(content.text(), "日本語のテキスト書く");
}

#[test]
fn erasing_words_follows_the_rule() {
    let text = "call(self.items)";
    for (rule, erase, at, kept) in [
        (
            WordRule::Code,
            Erase::WordBack,
            "call(self.items",
            "call(self.)",
        ),
        (
            WordRule::Prose,
            Erase::WordBack,
            "call(self.items",
            "call()",
        ),
        (WordRule::Code, Erase::WordForward, "call(", "call(.items)"),
        (WordRule::Prose, Erase::WordForward, "call(", "call()"),
    ] {
        let mut content = content_at(text, at.len());
        assert!(content.erase(erase, rule));
        assert_eq!(content.text(), kept, "{rule:?} {erase:?}");
    }
}

#[test]
fn word_steps_in_huge_line_stay_local() {
    const PHRASE: &str = "สวัสดีครับ lorem, ipsum ";
    let line = PHRASE.repeat((1 << 20) / PHRASE.len());
    let middle = PHRASE.len() * (line.len() / PHRASE.len() / 2);
    let mut content = content_at(&format!("{line}\nnext"), middle);

    content.travel(Travel::WordRight, false, None, WordRule::Prose);
    assert_eq!(content.caret(), middle + "สวัสดี".len());
    content.travel(Travel::WordLeft, false, None, WordRule::Prose);
    content.travel(Travel::WordLeft, false, None, WordRule::Prose);
    assert_eq!(content.caret(), middle - "ipsum ".len());
    assert_eq!(word_at(&content, middle + "สวัสดีครับ lo".len()), "lorem");
}

fn word_at(content: &Content, at: usize) -> String {
    let word = content.unit_at(at, Affinity::After, Granularity::Word, WordRule::Prose);
    content.text()[word].to_owned()
}

#[test]
fn deleting_forward_removes_a_whole_grapheme() {
    for text in [
        "e\u{301}x",
        "\u{1F1EF}\u{1F1F5}x",
        "\u{1F468}\u{200D}\u{1F469}x",
    ] {
        let mut content = content_at(text, 0);
        assert!(content.erase(Erase::Forward, WordRule::Prose));
        assert_eq!(content.text(), "x", "{text:?}");
    }
}
