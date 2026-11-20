use rikka_text_layout::{LayoutParams, Wrap};

use crate::support::{assert_close, layout_text, params, row_texts, wrapped};

const SPACING: f32 = 3.0;

fn spaced(params: LayoutParams) -> LayoutParams {
    LayoutParams {
        letter_spacing: SPACING,
        ..params
    }
}

fn widening(text: &str) -> f32 {
    layout_text(text, &spaced(params())).width() - layout_text(text, &params()).width()
}

#[test]
fn combining_mark_cluster_spaced_once() {
    assert_close(widening("e\u{301}a"), 2.0 * SPACING, "two clusters");
}

#[test]
fn wrapping_counts_letter_spacing() {
    let text = "spaced words wrap sooner";
    let room = layout_text(text, &params()).width();
    let plain = layout_text(text, &wrapped(room, Wrap::Word));
    let wide = layout_text(text, &spaced(wrapped(room, Wrap::Word)));
    assert_eq!(row_texts(&plain).len(), 1);
    assert!(row_texts(&wide).len() > 1, "{:?}", row_texts(&wide));
    for row in wide.rows() {
        assert!(row.width() <= room + 0.01);
    }
}
