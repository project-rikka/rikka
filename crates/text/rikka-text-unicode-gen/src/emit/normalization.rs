use std::collections::HashMap;

use super::trie;
use crate::error::{Error, Result};
use crate::model::Model;
use crate::rust::RustSource;
use crate::ucd::CODEPOINT_COUNT;

pub(crate) fn emit(model: &Model, sizes: &mut Vec<(&'static str, usize)>) -> Result<String> {
    let mut words = vec![0u32; CODEPOINT_COUNT];
    let mut decompositions = Vec::new();
    for (index, (&codepoint, &(first, second))) in model.decompositions.iter().enumerate() {
        words[codepoint as usize] = index_word(index)?;
        decompositions.push(first);
        decompositions.push(second.unwrap_or(0));
    }
    let mut starts = vec![0u16];
    let mut compositions = Vec::new();
    for (group, (&first, pairs)) in model.compositions.iter().enumerate() {
        words[first as usize] |= index_word(group)? << 16;
        for &(second, composite) in pairs {
            compositions.push(second);
            compositions.push(composite);
        }
        starts.push(
            u16::try_from(compositions.len() / 2)
                .map_err(|_| Error::Data("too many compositions".into()))?,
        );
    }
    let mut expansions = vec![0u16; CODEPOINT_COUNT];
    let mut expansion_starts = vec![0u16];
    let mut expansion_parts = Vec::new();
    let mut emitted: HashMap<&[u32], u16> = HashMap::new();
    for (&codepoint, expansion) in &model.compatibility {
        let index = match emitted.get(expansion.as_slice()) {
            Some(&index) => index,
            None => {
                expansion_parts.extend_from_slice(expansion);
                let index = u16::try_from(expansion_starts.len())
                    .map_err(|_| Error::Data("too many compatibility decompositions".into()))?;
                expansion_starts
                    .push(u16::try_from(expansion_parts.len()).map_err(|_| {
                        Error::Data("compatibility decompositions too long".into())
                    })?);
                emitted.insert(expansion, index);
                index
            }
        };
        expansions[codepoint as usize] = index;
    }
    let hex = |values: &[u32]| -> Vec<String> {
        values.iter().map(|value| format!("0x{value:x}")).collect()
    };
    let mut source = RustSource::default();
    source.line("use crate::trie::Trie;");
    let trie_bytes = trie::emit(&mut source, "NORMALIZATION", "u32", &words)?;
    source.array(
        "pub(crate) static DECOMPOSITIONS",
        "u32",
        hex(&decompositions),
    );
    source.array("pub(crate) static COMPOSITION_STARTS", "u16", &starts);
    source.array("pub(crate) static COMPOSITIONS", "u32", hex(&compositions));
    let compatibility_bytes = trie::emit(&mut source, "COMPATIBILITY", "u16", &expansions)?;
    source.array(
        "pub(crate) static COMPATIBILITY_STARTS",
        "u16",
        &expansion_starts,
    );
    source.array(
        "pub(crate) static COMPATIBILITY_PARTS",
        "u32",
        hex(&expansion_parts),
    );
    sizes.push(("normalization trie", trie_bytes));
    sizes.push((
        "compatibility decompositions",
        compatibility_bytes + expansion_starts.len() * 2 + expansion_parts.len() * 4,
    ));
    sizes.push((
        "decompositions and compositions",
        (decompositions.len() + compositions.len()) * 4 + starts.len() * 2,
    ));
    Ok(source.finish())
}

fn index_word(index: usize) -> Result<u32> {
    u16::try_from(index + 1)
        .map(u32::from)
        .map_err(|_| Error::Data("normalization index exceeds 16 bits".into()))
}
