use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

use crate::error::{Error, Result};
use crate::listing::Icon;

pub(crate) fn constants(type_name: &str, icons: &[Icon]) -> Result<String> {
    let mut named: BTreeMap<String, &Icon> = BTreeMap::new();
    let mut listed: BTreeMap<&str, char> = BTreeMap::new();
    for icon in icons {
        listed.insert(&icon.name, icon.codepoint);
        match named.entry(constant(&icon.name)?) {
            Entry::Vacant(vacant) => {
                vacant.insert(icon);
            }
            Entry::Occupied(occupied) if occupied.get().codepoint == icon.codepoint => {}
            Entry::Occupied(occupied) => {
                return Err(Error::ConstantCollision {
                    constant: occupied.key().clone(),
                    first: occupied.get().name.clone(),
                    second: icon.name.clone(),
                });
            }
        }
    }
    Ok(ConstantsImpl {
        type_name,
        named,
        listed,
    }
    .to_string())
}

struct ConstantsImpl<'a> {
    type_name: &'a str,
    named: BTreeMap<String, &'a Icon>,
    listed: BTreeMap<&'a str, char>,
}

impl fmt::Display for ConstantsImpl<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = self.type_name;
        writeln!(f, "use super::{type_name};\n\nimpl {type_name} {{")?;
        for (constant, icon) in &self.named {
            writeln!(
                f,
                "    pub const {constant}: Self = Self('\\u{{{:x}}}');",
                u32::from(icon.codepoint)
            )?;
        }
        writeln!(f, "}}\n")?;
        writeln!(f, "pub(super) static NAMED: &[(&str, {type_name})] = &[")?;
        for (name, codepoint) in &self.listed {
            writeln!(
                f,
                "    ({name:?}, {type_name}('\\u{{{:x}}}')),",
                u32::from(*codepoint)
            )?;
        }
        writeln!(f, "];")
    }
}

fn constant(name: &str) -> Result<String> {
    let valid = name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if name.is_empty() || !valid {
        return Err(Error::InvalidName(name.to_owned()));
    }
    let upper = name.to_ascii_uppercase().replace('-', "_");
    Ok(if upper.starts_with(|first: char| first.is_ascii_digit()) {
        format!("_{upper}")
    } else {
        upper
    })
}
