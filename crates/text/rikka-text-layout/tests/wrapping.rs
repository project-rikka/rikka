use rikka_text_layout::{Align, LayoutParams, Wrap};

use crate::support::{
    SIZE, assert_close, layout_text, natural_width, params, row_texts, shape, wrapped,
};

const PANGRAM: &str = "The quick brown fox jumps over the lazy dog while the cat sleeps";

#[test]
fn words_wrap_greedily_at_spaces() {
    let width = 120.0;
    let layout = layout_text(PANGRAM, &wrapped(width, Wrap::Word));
    let rows = row_texts(&layout);
    assert!(rows.len() > 2, "{rows:?}");
    assert_eq!(rows.concat(), PANGRAM);
    for (index, row) in layout.rows().iter().enumerate() {
        assert!(row.width() <= width, "row {index} is {} wide", row.width());
        let text = rows[index];
        assert!(index + 1 == rows.len() || text.ends_with(' '), "{text:?}");
        if let Some(next) = rows.get(index + 1) {
            let first_word = next.split(' ').next().unwrap_or_default();
            let longer = format!("{}{first_word}", text);
            assert!(
                natural_width(&longer) > width,
                "{first_word:?} would have fit after {text:?}"
            );
        }
    }
}

#[test]
fn glyph_wrap_breaks_url_between_graphemes() {
    let url = "https://example.com/a/very/long/path/that/does/not/fit/anywhere";
    let width = 90.0;
    let layout = layout_text(url, &wrapped(width, Wrap::Glyph));
    let rows = row_texts(&layout);
    assert!(rows.len() > 3, "{rows:?}");
    assert_eq!(rows.concat(), url);
    let rows = layout.rows();
    for row in rows {
        assert!(row.width() <= width + 0.01, "{}", row.width());
    }
    for row in &rows[..rows.len() - 1] {
        assert!(
            row.width() > width - 15.0,
            "rows are filled: {}",
            row.width()
        );
    }
}

#[test]
fn word_or_glyph_breaks_only_words_wider_than_the_row() {
    let text = "key 7802:9f8b:f82d:b932:b7eb:4c46:d479:b334 end";
    let width = natural_width("7802:9f8b:f82d");

    let broken = layout_text(text, &wrapped(width, Wrap::WordOrGlyph));
    let rows = row_texts(&broken);
    assert_eq!(rows.concat(), text);
    assert_eq!(rows.first(), Some(&"key "), "{rows:?}");
    assert_eq!(rows.last(), Some(&"end"), "{rows:?}");
    assert!(rows.len() > 3, "{rows:?}");
    for row in broken.rows() {
        assert!(row.width() <= width + 0.01, "{rows:?}");
    }

    let overflowing = layout_text(text, &wrapped(width, Wrap::Word));
    assert_eq!(
        row_texts(&overflowing),
        ["key ", "7802:9f8b:f82d:b932:b7eb:4c46:d479:b334 ", "end"]
    );
}

#[test]
fn min_content_width_is_the_widest_unbreakable_piece() {
    let text = "ab fingerprint mm";
    let cases = [
        (Wrap::None, natural_width(text)),
        (Wrap::Word, natural_width("fingerprint")),
        (Wrap::WordOrGlyph, natural_width("fingerprint")),
        (Wrap::Glyph, natural_width("m")),
    ];
    let paragraph = shape(text);
    for (wrap, expected) in cases {
        let least = paragraph.min_content_width(&LayoutParams { wrap, ..params() });
        assert_close(least, expected, &format!("{wrap:?} min-content"));
    }

    let least = paragraph.min_content_width(&params());
    let laid = paragraph.layout(&wrapped(least, Wrap::WordOrGlyph));
    assert_eq!(row_texts(&laid), ["ab ", "fingerprint ", "mm"]);
}

#[test]
fn japanese_breaks_between_ideographs_without_spaces() {
    let text = "日本語のテキストは単語の間にスペースがありませんが改行できます";
    let width = SIZE * 5.5;
    let layout = layout_text(text, &wrapped(width, Wrap::Word));
    let rows = row_texts(&layout);
    assert!(rows.len() >= 5, "{rows:?}");
    assert_eq!(rows.concat(), text);
    for (row, text) in layout.rows().iter().zip(&rows) {
        assert!(row.width() <= width + 0.01);
        assert!(text.chars().count() <= 5, "{text:?}");
    }
    assert!(
        rows[..rows.len() - 1]
            .iter()
            .all(|row| row.chars().count() == 5)
    );
}

#[test]
fn trailing_spaces_hang() {
    let text = "hello      world";
    let width = natural_width("hello") + 4.0;
    let layout = layout_text(text, &wrapped(width, Wrap::Word));
    let rows = row_texts(&layout);
    assert_eq!(rows, ["hello      ", "world"]);
    let first = &layout.rows()[0];
    assert_close(first.width(), natural_width("hello"), "hanging row width");
    let glyphs = layout.glyphs(first);
    let last = glyphs.last().unwrap();
    assert!(
        last.x + last.advance > width,
        "the spaces stand past the edge"
    );
}

#[test]
fn justified_rows_fill_width_except_last() {
    let width = 150.0;
    let layout = layout_text(
        PANGRAM,
        &LayoutParams {
            align: Some(Align::Justified),
            ..wrapped(width, Wrap::Word)
        },
    );
    let rows = layout.rows();
    assert!(rows.len() > 2);
    for row in &rows[..rows.len() - 1] {
        assert_close(row.left(), 0.0, "justified left edge");
        assert_close(row.width(), width, "justified width");
        let right = layout
            .glyphs(row)
            .iter()
            .filter(|glyph| !layout.text()[glyph.cluster()].trim().is_empty())
            .map(|glyph| glyph.x + glyph.advance)
            .fold(0.0_f32, f32::max);
        assert_close(right, width, "last letter touches the right edge");
    }
    let last = rows.last().unwrap();
    assert!(last.width() < width);
}
