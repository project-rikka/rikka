use super::layout::{Layout, bits_for};
use super::trie;
use crate::error::{Error, Result};
use crate::model::Model;
use crate::rust::RustSource;
use crate::ucd::CODEPOINT_COUNT;

pub(crate) fn emit(model: &Model, sizes: &mut Vec<(&'static str, usize)>) -> Result<String> {
    let mut layout = Layout::default();
    let columns = [
        ("INDIC_SYLLABIC_CATEGORY", &model.indic_syllabic_category),
        (
            "INDIC_POSITIONAL_CATEGORY",
            &model.indic_positional_category,
        ),
        ("JOINING_GROUP", &model.joining_group),
    ]
    .map(|(name, column)| {
        (
            layout.field(name, column.enumeration.bits()),
            &column.values,
        )
    });
    let extensions_shift = layout.field(
        "SCRIPT_EXTENSIONS",
        bits_for(model.script_extension_sets.len() + 1),
    );
    if layout.bits() > 32 {
        return Err(Error::Data(format!(
            "the shaping word needs {} bits",
            layout.bits()
        )));
    }
    let words: Vec<u32> = (0..CODEPOINT_COUNT)
        .map(|codepoint| {
            columns.iter().fold(0u32, |word, (shift, values)| {
                word | u32::from(values[codepoint]) << shift
            }) | u32::from(model.script_extensions[codepoint]) << extensions_shift
        })
        .collect();
    let mut source = RustSource::default();
    source.line("use crate::trie::Trie;");
    source.blank();
    layout.emit(&mut source, "u32");
    let bytes = trie::emit_indexed(&mut source, "SHAPING", "u32", &words, |word| {
        word.to_string()
    })?;
    sizes.push(("shaping trie and words", bytes));
    Ok(source.finish())
}
