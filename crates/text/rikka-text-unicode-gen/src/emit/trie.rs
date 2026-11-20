use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;

use crate::error::{Error, Result};
use crate::rust::RustSource;
use crate::trie::Trie;

pub(crate) fn emit<T: Copy + Eq + Hash + Display>(
    source: &mut RustSource,
    name: &str,
    value_type: &str,
    values: &[T],
) -> Result<usize> {
    let trie = Trie::smallest(values)?;
    source.blank();
    source.assignment(
        &format!(
            "pub(crate) const {name}: Trie<{value_type}, {}, {}>",
            trie.leaf_bits, trie.mid_bits
        ),
        &format!("Trie::new(&{name}_TOP, &{name}_MID, &{name}_LEAF)"),
    );
    source.array(&format!("static {name}_TOP"), "u16", &trie.top);
    source.array(&format!("static {name}_MID"), "u16", &trie.mid);
    source.array(&format!("static {name}_LEAF"), value_type, &trie.leaf);
    Ok(trie.bytes())
}

pub(crate) fn emit_indexed<T: Copy + Eq + Hash>(
    source: &mut RustSource,
    name: &str,
    value_type: &str,
    values: &[T],
    format: impl Fn(T) -> String,
) -> Result<usize> {
    let (distinct, trie_bytes) = emit_index(source, name, values)?;
    source.array(
        &format!("pub(crate) static {name}_VALUES"),
        value_type,
        distinct.iter().map(|&value| format(value)),
    );
    Ok(trie_bytes + distinct.len() * size_of::<T>())
}

pub(crate) fn emit_index<T: Copy + Eq + Hash>(
    source: &mut RustSource,
    name: &str,
    values: &[T],
) -> Result<(Vec<T>, usize)> {
    let mut distinct: Vec<T> = Vec::new();
    let mut positions: HashMap<T, u16> = HashMap::new();
    let mut indices = Vec::with_capacity(values.len());
    for &value in values {
        let position = match positions.get(&value) {
            Some(&position) => position,
            None => {
                let position = u16::try_from(distinct.len())
                    .map_err(|_| Error::Data(format!("{name} has too many distinct values")))?;
                distinct.push(value);
                positions.insert(value, position);
                position
            }
        };
        indices.push(position);
    }
    let trie_bytes = emit(source, name, "u16", &indices)?;
    Ok((distinct, trie_bytes))
}
