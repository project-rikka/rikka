mod editing;
mod fonts;
mod layout;
mod shaping;
mod style;

pub use editing::{
    Affinity, Caret, Cursor, Movement, SelectionRect, TextSource, next_grapheme_boundary,
    next_word_boundary, previous_grapheme_boundary, previous_word_boundary, word_range_at,
};
pub use fonts::LocationId;
pub use layout::{
    Align, Decoration, DecorationKind, Glyph, LayoutParams, ParagraphLayout, Row, Wrap,
};
pub use rikka_text_catalog::{FaceId, Family, FontStyle, Weight, Width};
pub use rikka_text_font::Synthesis;
pub use rikka_text_unicode::{Level, WordRule};
pub use shaping::{Paragraph, ShapedParagraph};
pub use style::{Caps, StyleSpan, TextStyle};
