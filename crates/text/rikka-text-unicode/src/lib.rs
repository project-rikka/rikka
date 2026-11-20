mod bidi;
mod emoji;
mod general_category;
mod mirroring;
mod normalization;
mod properties;
mod script;
mod script_runs;
mod segmentation;
mod shaping;
mod tables;
mod trie;
mod version;

pub use bidi::{
    BaseDirection, Bidi, BidiParagraph, Direction, Level, VisualLine, VisualRun, logical_to_visual,
    visual_to_logical,
};
pub use emoji::{
    EMOJI_PRESENTATION_SELECTOR, EmojiPresentation, EmojiSequence, EmojiSequenceKind,
    TEXT_PRESENTATION_SELECTOR, basic_emoji, emoji_presentation, emoji_sequence,
};
pub use mirroring::{PairedBracket, is_bidi_mirrored, mirroring_glyph, paired_bracket};
pub use normalization::{
    Decomposition, canonical_decomposition, compose, decompose, is_nfc, is_nfd, nfc, nfc_into, nfd,
    nfd_into, nfkc, nfkc_into, nfkd, nfkd_into,
};
pub use properties::{
    BracketType, Properties, bidi_class, combining_class, east_asian_width, general_category,
    grapheme_cluster_break, indic_conjunct_break, is_default_ignorable, is_emoji,
    is_emoji_component, is_emoji_modifier, is_emoji_modifier_base, is_emoji_presentation,
    is_extended_pictographic, is_regional_indicator, joining_type, line_break_class, properties,
    script, word_break,
};
pub use script::{OpenTypeTag, has_script, script_extensions};
pub use script_runs::{ScriptRun, ScriptRuns, script_runs};
pub use segmentation::{
    BreakOpportunity, DictionaryBreaks, Graphemes, LineBreak, LineBreaks, WordRule, WordSegment,
    WordSegments, dictionary_breaks, graphemes, line_breaks, needs_dictionary, word_segments,
};
pub use shaping::{indic_positional_category, indic_syllabic_category, joining_group};
pub use tables::enums::{
    BidiClass, EastAsianWidth, GeneralCategory, GraphemeClusterBreak, IndicConjunctBreak,
    IndicPositionalCategory, IndicSyllabicCategory, JoiningGroup, JoiningType, LineBreakClass,
    Script, WordBreak,
};
pub use version::{UNICODE_VERSION, UnicodeVersion};
