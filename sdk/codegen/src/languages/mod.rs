pub(crate) mod c;
pub(crate) mod cpp;
pub(crate) mod csharp;
mod doxygen;
pub(crate) mod go;
pub(crate) mod java;
mod keywords;
mod projection;
pub(crate) mod python;
pub(crate) mod rust;

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, Span};

fn unique(
    names: impl IntoIterator<Item = (String, Span)>,
    message: impl Fn(&str) -> String,
) -> Result<(), Diagnostic> {
    let mut seen = BTreeSet::new();
    for (name, span) in names {
        if !seen.insert(name.clone()) {
            return Err(Diagnostic::new(span, message(&name)));
        }
    }
    Ok(())
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
