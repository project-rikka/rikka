use std::fmt::Write as _;

use crate::list::{Kind, Rules, Section};

pub(crate) fn table(rules: &Rules) -> String {
    let mut names = String::new();
    let mut ends = Vec::with_capacity(rules.len());
    for name in rules.keys() {
        names.push_str(name);
        ends.push(names.len());
    }
    let mut out = String::new();
    for (constant, value) in [
        ("ICANN", Section::Icann.shift()),
        ("PRIVATE", Section::Private.shift()),
        ("NORMAL", Kind::Normal.bit()),
        ("WILDCARD", Kind::Wildcard.bit()),
        ("EXCEPTION", Kind::Exception.bit()),
    ] {
        writeln!(out, "pub(crate) const {constant}: u8 = {value};").expect("writing to a String");
    }
    writeln!(out, "pub(crate) static NAMES: &str = {names:?};").expect("writing to a String");
    list(&mut out, "ENDS", "u32", ends.iter());
    list(&mut out, "FLAGS", "u8", rules.values());
    out
}

fn list<T: std::fmt::Display>(
    out: &mut String,
    name: &str,
    element: &str,
    values: impl ExactSizeIterator<Item = T>,
) {
    let count = values.len();
    writeln!(out, "pub(crate) static {name}: [{element}; {count}] = [")
        .expect("writing to a String");
    for value in values {
        writeln!(out, "{value},").expect("writing to a String");
    }
    writeln!(out, "];").expect("writing to a String");
}
