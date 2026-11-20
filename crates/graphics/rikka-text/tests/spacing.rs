use std::sync::Arc;

use rikka_text::{TextLayout, TextSpec, Wrap};

const SPACING: f32 = 2.5;
const WORDS: &str = "tracked caption";

fn plain_and_spaced(text: &str) -> (Arc<TextLayout>, Arc<TextLayout>) {
    let spec = TextSpec::new(text, 18.0);
    let spaced = spec.clone().letter_spacing(SPACING);
    (spec.layout(None), spaced.layout(None))
}

fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{what}: {actual} is not {expected}"
    );
}

#[test]
fn spacing_is_part_of_layout_key() {
    let words = "a caption laid once per spacing";
    let (plain, spaced) = plain_and_spaced(words);
    let again = TextSpec::new(words, 18.0)
        .letter_spacing(SPACING)
        .layout(None);

    assert!(!Arc::ptr_eq(&plain, &spaced));
    assert!(Arc::ptr_eq(&spaced, &again));
    assert!(Arc::ptr_eq(plain.shaped(), spaced.shaped()), "shaped once");
    assert_eq!(spaced.letter_spacing(), SPACING);
    assert_close(
        spaced.widest(),
        plain.widest() + words.chars().count() as f32 * SPACING,
        "every letter is spaced",
    );
}

#[test]
fn carets_hits_and_glyphs_include_spacing() {
    let (plain, spaced) = plain_and_spaced(WORDS);
    for index in 0..WORDS.len() {
        let shift = index as f32 * SPACING;
        let caret = spaced.caret(spaced.cursor_of(index));
        assert_close(
            caret.x,
            plain.caret(plain.cursor_of(index)).x + shift,
            "caret",
        );
        let hit = spaced.hit(caret.x + 0.2, caret.top + 1.0);
        assert_eq!(spaced.offset_of(hit), index);
    }
    let plain_row = plain.rows().next().expect("first row");
    let spaced_row = spaced.rows().next().expect("first row");
    for (index, (before, after)) in plain_row
        .glyphs()
        .iter()
        .zip(spaced_row.glyphs())
        .enumerate()
    {
        assert_close(after.x, before.x + index as f32 * SPACING, "glyph");
    }
    let placed: Vec<_> = spaced_row.placed((0.0, 0.0), 2.0).collect();
    let last = placed.last().expect("glyph present");
    let last_glyph = spaced_row.glyphs().last().expect("glyph present");
    assert!((last.x as f32 - last_glyph.x * 2.0).abs() <= 1.0);

    assert_close(spaced_row.width(), spaced.widest(), "row width");
    let selected: f32 = spaced
        .selection_rects(0..WORDS.len())
        .into_iter()
        .map(|(_, rect)| rect.width())
        .sum();
    assert_close(selected, spaced.widest(), "selection");
}

#[test]
fn spacing_changes_wrapping() {
    let (plain, _) = plain_and_spaced(WORDS);
    let room = Some(plain.widest());
    let spec = TextSpec::new(WORDS, 18.0).wrap(Wrap::Word);
    assert_eq!(spec.clone().layout(room).row_count(), 1);
    assert_eq!(spec.letter_spacing(SPACING).layout(room).row_count(), 2);
}
