use std::collections::HashSet;

use crate::code::{Code, quoted};
use crate::model::Enum;
use crate::names;

pub(crate) fn emit(code: &mut Code, definition: &Enum) {
    if definition.bitfield {
        bitfield(code, definition);
    } else {
        enumeration(code, definition);
    }
    if definition.name == "error" && !definition.bitfield {
        error(code, definition);
    }
}

fn bitfield(code: &mut Code, definition: &Enum) {
    let name = names::camel(&definition.name);
    code.open("bitflags::bitflags! {")
        .line("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
        .open(format!("pub struct {name}: u32 {{"));
    for entry in definition.entries.iter().filter(|entry| entry.value != 0) {
        code.line(format!(
            "const {} = {};",
            names::screaming_snake(&entry.name),
            entry.value
        ));
    }
    code.close("}").close("}").blank();
}

fn enumeration(code: &mut Code, definition: &Enum) {
    let name = names::camel(&definition.name);
    code.line("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
        .open(format!("pub enum {name} {{"));
    for entry in &definition.entries {
        code.line(format!("{},", names::camel(&entry.name)));
    }
    code.close("}").blank();

    code.open(format!("impl TryFrom<u32> for {name} {{"))
        .line("type Error = wire::UnknownValue;")
        .blank()
        .open("fn try_from(value: u32) -> Result<Self, Self::Error> {")
        .open("match value {");
    let mut values = HashSet::new();
    for entry in &definition.entries {
        if values.insert(entry.value) {
            code.line(format!(
                "{} => Ok(Self::{}),",
                entry.value,
                names::camel(&entry.name)
            ));
        }
    }
    code.line("_ => Err(wire::UnknownValue(value)),")
        .close("}")
        .close("}")
        .close("}")
        .blank();

    code.open(format!("impl From<{name}> for u32 {{"))
        .open(format!("fn from(value: {name}) -> u32 {{"))
        .open("match value {");
    for entry in &definition.entries {
        code.line(format!(
            "{name}::{} => {},",
            names::camel(&entry.name),
            entry.value
        ));
    }
    code.close("}").close("}").close("}").blank();
}

fn error(code: &mut Code, definition: &Enum) {
    code.open("impl Error {")
        .open("pub fn code(self) -> u32 {")
        .line("u32::from(self)")
        .close("}")
        .blank()
        .open("pub fn summary(self) -> &'static str {")
        .open("match self {");
    for entry in &definition.entries {
        let summary = entry
            .summary
            .clone()
            .unwrap_or_else(|| entry.name.replace('_', " "));
        code.line(format!(
            "Self::{} => {},",
            names::camel(&entry.name),
            quoted(&summary)
        ));
    }
    code.close("}").close("}").close("}").blank();
}
