use rikka_text_layout::{LayoutParams, ParagraphLayout, Wrap};

use crate::support::{assert_close, family, layout_text, natural_width, params};

const MARK: &str = "\u{2026}";

fn elided(text: &str, width: f32) -> ParagraphLayout {
    layout_text(
        text,
        &LayoutParams {
            width: Some(width),
            wrap: Wrap::None,
            ellipsize: true,
            ..params()
        },
    )
}

fn assert_fits(layout: &ParagraphLayout, width: f32) {
    assert_eq!(layout.rows().len(), 1);
    let row = &layout.rows()[0];
    assert!(row.is_elided(), "{:?}", layout.visible_text());
    assert!(row.width() <= width + 0.01, "{} > {width}", row.width());
    let glyphs = layout.glyphs(row);
    let left = glyphs
        .iter()
        .map(|glyph| glyph.x)
        .fold(f32::INFINITY, f32::min);
    let right = glyphs
        .iter()
        .map(|glyph| glyph.x + glyph.advance)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        left >= -0.01 && right <= width + 0.01,
        "{left}..{right} in {width}"
    );
    let visible = layout.visible_text();
    assert!(visible.ends_with(MARK), "{visible:?}");
    let prefix = &visible[..visible.len() - MARK.len()];
    assert!(layout.text().starts_with(prefix));
    assert_eq!(prefix, prefix.trim_end(), "trailing spaces are trimmed");
    assert_eq!(row.text_range(), 0..prefix.len());
}

fn mark_glyph_x(layout: &ParagraphLayout) -> (f32, f32, f32) {
    let row = &layout.rows()[0];
    let glyphs = layout.glyphs(row);
    let mark = glyphs
        .iter()
        .find(|glyph| glyph.start == glyph.end)
        .expect("no ellipsis glyph drawn");
    let others = glyphs.iter().filter(|glyph| glyph.start != glyph.end);
    let min = others
        .clone()
        .map(|glyph| glyph.x)
        .fold(f32::INFINITY, f32::min);
    let max = others
        .map(|glyph| glyph.x)
        .fold(f32::NEG_INFINITY, f32::max);
    (mark.x, min, max)
}

#[test]
fn ltr_ellipsis_cuts_at_grapheme_and_fits() {
    let text = "The quick brown fox jumps over the lazy dog";
    for width in [60.0, 97.0, 150.0, 233.0] {
        let layout = elided(text, width);
        assert_fits(&layout, width);
        let visible = layout.visible_text();
        let drawn = natural_width(&visible);
        assert!(
            (layout.width() - drawn).abs() < 0.5,
            "{} vs {drawn}",
            layout.width()
        );
        let prefix = &visible[..visible.len() - MARK.len()];
        let next = &text[..text[prefix.len()..]
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
            .map_or(text.len(), |(offset, character)| {
                prefix.len() + offset + character.len_utf8()
            })];
        assert!(
            natural_width(&format!("{next}{MARK}")) > width - 0.5,
            "{next:?} would also have fit in {width}"
        );
    }
    let (mark, _, max) = mark_glyph_x(&elided(text, 120.0));
    assert!(mark > max, "the ellipsis ends the row");
}

#[test]
fn rtl_ellipsis_sits_on_left() {
    let text = "\u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{5E2}\u{5D5}\u{5DC}\u{5DD} \u{5D6}\u{5D4}\u{5D5} \u{5DE}\u{5E9}\u{5E4}\u{5D8} \u{5D0}\u{5E8}\u{5D5}\u{5DA} \u{5DE}\u{5D0}\u{5D5}\u{5D3}";
    let width = 110.0;
    let layout = elided(text, width);
    assert_fits(&layout, width);
    let (mark, min, _) = mark_glyph_x(&layout);
    assert!(mark < min, "the ellipsis is at the visual end");
    let row = &layout.rows()[0];
    assert_close(row.right(), width, "right aligned");
}

#[test]
fn ellipsis_uses_requested_font() {
    let marks: Vec<(&str, u16, f32)> = [
        "このプロジェクトの上で動くエージェント",
        "main以外のworktreeを片付ける",
        "The quick brown fox jumps over the lazy dog",
    ]
    .into_iter()
    .map(|text| {
        let layout = elided(text, 120.0);
        let row = &layout.rows()[0];
        let mark = *layout
            .glyphs(row)
            .iter()
            .find(|glyph| glyph.start == glyph.end)
            .expect("no ellipsis glyph drawn");
        (family(mark.face), mark.id, mark.advance)
    })
    .collect();
    assert_eq!(marks[0].0, "Inter", "{marks:?}");
    assert!(
        marks.windows(2).all(|pair| pair[0] == pair[1]),
        "every cut row ends in one ellipsis: {marks:?}"
    );
}
