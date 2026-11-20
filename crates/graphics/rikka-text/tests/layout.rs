use std::sync::Arc;

use rikka_text::{TextSpec, Wrap, layout, layout_approx};

const LINES: &str = "first line\nsecond line\nthird line";

const WORDS: &str = "shaped once and laid out again for every width it is asked for";

#[test]
fn rows_span_paragraphs_for_every_line_ending() {
    for (text, expected) in [
        (LINES, vec![(0, 0..10), (1, 11..22), (2, 23..33)]),
        ("one\r\ntwo", vec![(0, 0..3), (1, 5..8)]),
        (
            "one\rtwo\n\nthree",
            vec![(0, 0..3), (1, 4..7), (2, 8..8), (3, 9..14)],
        ),
    ] {
        let layout = TextSpec::new(text, 14.0).layout(None);
        let rows: Vec<_> = layout
            .rows()
            .map(|row| (row.line(), row.text_range()))
            .collect();

        assert_eq!(rows, expected, "{text:?}");
        assert_eq!(layout.row_count(), expected.len());
    }
}

#[test]
fn hit_resolves_within_paragraph_or_text_ends() {
    let layout = TextSpec::new(LINES, 14.0).layout(None);
    let caret = layout.caret(layout.cursor_of(15));
    let cursor = layout.hit(caret.x + 0.25, caret.top + caret.height / 2.0);

    assert_eq!(caret.row, 1);
    assert_eq!(cursor.line, 1);
    assert_eq!(layout.offset_of(cursor), 15);
    assert_eq!(layout.offset_of(layout.hit(30.0, -4.0)), 0);
    assert_eq!(
        layout.offset_of(layout.hit(0.0, layout.height() + 4.0)),
        LINES.len()
    );
}

#[test]
fn selection_rects_cover_every_row() {
    let layout = TextSpec::new(LINES, 14.0).layout(None);
    let rows = |range| -> Vec<usize> {
        let mut rows: Vec<usize> = layout
            .selection_rects(range)
            .into_iter()
            .map(|(row, _)| row)
            .collect();
        rows.dedup();
        rows
    };
    assert_eq!(rows(0..5), [0]);
    assert_eq!(rows(11..17), [1]);
    assert_eq!(rows(6..17), [0, 1]);
    assert_eq!(rows(3..30), [0, 1, 2]);
    assert!(rows(4..4).is_empty());

    let text = "a wrapped line of words\n".repeat(40);
    let wrapped = TextSpec::new(text.as_str(), 14.0)
        .line_height(17.3)
        .layout(Some(60.0));
    let tops: Vec<f32> = wrapped.rows().map(|row| row.top()).collect();
    assert!(wrapped.row_count() > 40, "lines wrap");
    for (row, band) in wrapped.selection_rects(0..text.len()) {
        assert!((tops[row] - band.y()).abs() < 1e-3, "row {row}");
    }
}

#[test]
fn relayout_at_narrower_width_reuses_shaping() {
    let spec = TextSpec::new(format!("{WORDS} wide"), 14.0);
    let wide = layout(spec.as_shape(), Some(400.0));
    let narrow = layout(spec.as_shape(), Some(120.0));

    assert!(narrow.row_count() > wide.row_count());
    assert!(Arc::ptr_eq(wide.shaped(), narrow.shaped()), "no reshaping");
}

#[test]
fn compatible_width_reuses_layout() {
    let spec = TextSpec::new(format!("{WORDS} known"), 14.0);
    let measured = layout(spec.as_shape(), Some(300.5));

    let again = layout(spec.as_shape(), Some(measured.widest()));
    let rounded_up = layout_approx(spec.as_shape(), Some(300.9));

    assert!(Arc::ptr_eq(&measured, &again), "same line breaks");
    assert!(
        Arc::ptr_eq(&measured, &rounded_up),
        "the draw uses the measure"
    );
}

#[test]
fn overflowing_line_ellipsizes_without_reshaping() {
    let spec = TextSpec::new(format!("{WORDS} cut"), 14.0)
        .wrap(Wrap::None)
        .ellipsize(true);
    let whole = layout(spec.as_shape(), None);
    let cut = layout(spec.as_shape(), Some(whole.widest() / 2.0));

    assert!(Arc::ptr_eq(whole.shaped(), cut.shaped()), "no reshaping");
    assert!(cut.is_elided());
    assert!(
        cut.visible_text().ends_with('\u{2026}'),
        "{}",
        cut.visible_text()
    );
    assert!(cut.widest() <= whole.widest() / 2.0 + 0.01);
    assert!(!whole.is_elided());

    let snug = layout(spec.as_shape(), Some(whole.widest().ceil() - 0.5));
    assert!(
        !snug.is_elided(),
        "a line within a pixel of its room is left whole: {}",
        snug.visible_text()
    );
}
