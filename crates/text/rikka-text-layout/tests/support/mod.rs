use rikka_text_catalog::catalog;
use rikka_text_layout::{
    FaceId, LayoutParams, Paragraph, ParagraphLayout, ShapedParagraph, TextStyle, Wrap,
};

pub(crate) const SIZE: f32 = 16.0;
pub(crate) const LINE_HEIGHT: f32 = 22.0;

pub(crate) fn inter() -> TextStyle {
    TextStyle::new(["Inter"])
}

pub(crate) fn shape(text: &str) -> ShapedParagraph {
    Paragraph::new(text, &inter()).shape()
}

pub(crate) fn params() -> LayoutParams {
    LayoutParams::new(SIZE, LINE_HEIGHT)
}

pub(crate) fn wrapped(width: f32, wrap: Wrap) -> LayoutParams {
    LayoutParams {
        width: Some(width),
        wrap,
        ..params()
    }
}

pub(crate) fn layout_text(text: &str, params: &LayoutParams) -> ParagraphLayout {
    shape(text).layout(params)
}

pub(crate) fn row_texts(layout: &ParagraphLayout) -> Vec<&str> {
    layout
        .rows()
        .iter()
        .map(|row| &layout.text()[row.text_range()])
        .collect()
}

pub(crate) fn family(face: FaceId) -> &'static str {
    catalog().face(face).family()
}

pub(crate) fn natural_width(text: &str) -> f32 {
    layout_text(text, &params()).width()
}

pub(crate) fn assert_close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01,
        "{what}: {actual} is not {expected}"
    );
}
