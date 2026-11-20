use std::fmt::{self, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::document::{Node, Value};

const WRAP: usize = 96;

pub(crate) fn write(nodes: &[Node], version: u32) -> Vec<u8> {
    let mut out = String::new();
    write_document(&mut out, nodes, version).expect("formatting into a String is infallible");
    out.into_bytes()
}

fn write_document(out: &mut String, nodes: &[Node], version: u32) -> fmt::Result {
    write!(
        out,
        "; FBX {}.{}.{} project file\n; \
         ----------------------------------------------------\n\n",
        version / 1000,
        version % 1000 / 100,
        version % 100 / 10
    )?;
    for node in nodes {
        record(out, node, 0)?;
        out.push('\n');
    }
    Ok(())
}

fn record(out: &mut String, node: &Node, depth: usize) -> fmt::Result {
    indent(out, depth);
    out.push_str(&node.name);
    out.push(':');

    for (index, value) in node.values.iter().enumerate() {
        out.push(match index {
            0 => ' ',
            _ => ',',
        });
        write_value(out, value, depth)?;
    }

    if node.children.is_empty() {
        out.push('\n');
        return Ok(());
    }
    out.push_str(" {\n");
    for child in &node.children {
        record(out, child, depth + 1)?;
    }
    indent(out, depth);
    out.push_str("}\n");
    Ok(())
}

fn write_value(out: &mut String, value: &Value, depth: usize) -> fmt::Result {
    match value {
        Value::Bool(flag) => {
            out.push_str(match flag {
                true => "1",
                false => "0",
            });
            Ok(())
        }
        Value::I16(number) => integer(out, i64::from(*number)),
        Value::I32(number) => integer(out, i64::from(*number)),
        Value::I64(number) => integer(out, *number),
        Value::F32(number) => real(out, f64::from(*number)),
        Value::F64(number) => real(out, *number),
        Value::Text(text) | Value::Word(text) => {
            quoted(out, &named(text));
            Ok(())
        }
        Value::Raw(bytes) => {
            quoted(out, &STANDARD.encode(bytes));
            Ok(())
        }
        Value::Bools(flags) => array(out, depth, flags, |out, &flag| {
            integer(out, i64::from(flag))
        }),
        Value::I32s(numbers) => array(out, depth, numbers, |out, &number| {
            integer(out, i64::from(number))
        }),
        Value::I64s(numbers) => array(out, depth, numbers, |out, &number| integer(out, number)),
        Value::F32s(numbers) => array(out, depth, numbers, |out, &number| {
            real(out, f64::from(number))
        }),
        Value::F64s(numbers) => array(out, depth, numbers, |out, &number| real(out, number)),
    }
}

fn array<T>(
    out: &mut String,
    depth: usize,
    values: &[T],
    write: impl Fn(&mut String, &T) -> fmt::Result,
) -> fmt::Result {
    writeln!(out, "*{} {{", values.len())?;
    indent(out, depth + 1);
    out.push_str("a: ");
    let mut column = out.len();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        if out.len() - column > WRAP {
            out.push('\n');
            indent(out, depth + 2);
            column = out.len();
        }
        write(out, value)?;
    }
    out.push('\n');
    indent(out, depth);
    out.push('}');
    Ok(())
}

fn integer(out: &mut String, value: i64) -> fmt::Result {
    write!(out, "{value}")
}

fn real(out: &mut String, value: f64) -> fmt::Result {
    if !value.is_finite() {
        out.push_str("0.0");
        return Ok(());
    }
    let start = out.len();
    write!(out, "{value}")?;
    if !out[start..].contains(['.', 'e', 'E']) {
        out.push_str(".0");
    }
    Ok(())
}

fn quoted(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&cr;"),
            '\n' => out.push_str("&lf;"),
            character => out.push(character),
        }
    }
    out.push('"');
}

fn named(text: &str) -> String {
    match text.split_once('\u{0}') {
        Some((name, class)) => format!("{}::{name}", class.trim_start_matches('\u{1}')),
        None => text.to_owned(),
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push('\t');
    }
}
