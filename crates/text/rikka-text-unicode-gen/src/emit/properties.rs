use super::layout::{Layout, bits_for};
use super::trie;
use crate::error::{Error, Result};
use crate::model::Model;
use crate::rust::RustSource;
use crate::ucd::CODEPOINT_COUNT;

pub(crate) fn emit(
    model: &Model,
    line_break_columns: &[u8],
    sizes: &mut Vec<(&'static str, usize)>,
) -> Result<String> {
    let mut layout = Layout::default();
    let columns = [
        ("GENERAL_CATEGORY", &model.general_category),
        ("SCRIPT", &model.script),
        ("BIDI_CLASS", &model.bidi_class),
        ("LINE_BREAK", &model.line_break),
        ("GRAPHEME_CLUSTER_BREAK", &model.grapheme_cluster_break),
        ("WORD_BREAK", &model.word_break),
        ("EAST_ASIAN_WIDTH", &model.east_asian_width),
        ("INDIC_CONJUNCT_BREAK", &model.indic_conjunct_break),
        ("JOINING_TYPE", &model.joining_type),
    ]
    .map(|(name, column)| {
        let shift = layout.field(name, column.enumeration.bits());
        (shift, &column.values)
    });
    let bracket_shift = layout.field("BRACKET_TYPE", bits_for(3));
    let combining_shift = layout.field("COMBINING_CLASS", 8);
    let flags = &model.flags;
    let has_extensions: Vec<bool> = model
        .script_extensions
        .iter()
        .map(|&set| set != 0)
        .collect();
    let flag_columns = [
        ("EXTENDED_PICTOGRAPHIC", &flags.extended_pictographic),
        ("EMOJI", &flags.emoji),
        ("EMOJI_PRESENTATION", &flags.emoji_presentation),
        ("EMOJI_MODIFIER", &flags.emoji_modifier),
        ("EMOJI_MODIFIER_BASE", &flags.emoji_modifier_base),
        ("EMOJI_COMPONENT", &flags.emoji_component),
        ("DEFAULT_IGNORABLE", &flags.default_ignorable),
        ("BIDI_MIRRORED", &flags.bidi_mirrored),
        ("HAS_SCRIPT_EXTENSIONS", &has_extensions),
        ("DECOMPOSES", &flags.decomposes),
        ("NFC_NO", &flags.nfc_no),
        ("NFC_MAYBE", &flags.nfc_maybe),
    ]
    .map(|(name, column)| (layout.field(name, 1), column));
    if layout.bits() > 64 {
        return Err(Error::Data(format!(
            "the property word needs {} bits",
            layout.bits()
        )));
    }

    let mut words = Vec::with_capacity(CODEPOINT_COUNT);
    for codepoint in 0..CODEPOINT_COUNT {
        let mut word = 0u64;
        for (shift, values) in &columns {
            word |= u64::from(values[codepoint]) << shift;
        }
        word |= u64::from(model.bracket_type[codepoint]) << bracket_shift;
        word |= u64::from(model.combining_class[codepoint]) << combining_shift;
        for (shift, values) in &flag_columns {
            word |= u64::from(values[codepoint]) << shift;
        }
        words.push((word, line_break_columns[codepoint]));
    }

    let mut source = RustSource::default();
    source.line("use crate::trie::Trie;");
    source.blank();
    layout.emit(&mut source, "u64");
    let (distinct, trie_bytes) = trie::emit_index(&mut source, "PROPERTIES", &words)?;
    source.array(
        "pub(crate) static PROPERTIES_VALUES",
        "u64",
        distinct.iter().map(|(word, _)| format!("0x{word:x}")),
    );
    source.array(
        "pub(crate) static LINE_BREAK_COLUMNS",
        "u8",
        distinct.iter().map(|(_, column)| column),
    );
    sizes.push((
        "property trie, words and line break columns",
        trie_bytes + distinct.len() * 9,
    ));
    Ok(source.finish())
}
