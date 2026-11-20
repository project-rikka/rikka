use std::cmp::Ordering;

use super::value::Value;

const UNSIGNED: u8 = 0;
const NEGATIVE: u8 = 1;
const BYTES: u8 = 2;
const TEXT: u8 = 3;
const ARRAY: u8 = 4;
const MAP: u8 = 5;
const TAG: u8 = 6;

const FALSE: u8 = 0xf4;
const TRUE: u8 = 0xf5;
const NULL: u8 = 0xf6;
const UNDEFINED: u8 = 0xf7;
const SINGLE: u8 = 0xfa;
const DOUBLE: u8 = 0xfb;

pub(super) fn write(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Unsigned(n) => head(UNSIGNED, *n, out),
        Value::Negative(n) => head(NEGATIVE, *n, out),
        Value::Bytes(bytes) => {
            head(BYTES, bytes.len() as u64, out);
            out.extend_from_slice(bytes);
        }
        Value::Text(text) => {
            head(TEXT, text.len() as u64, out);
            out.extend_from_slice(text.as_bytes());
        }
        Value::Array(items) => {
            head(ARRAY, items.len() as u64, out);
            for item in items {
                write(item, out);
            }
        }
        Value::Map(entries) => write_map(entries, out),
        Value::Tag(tag, inner) => {
            head(TAG, *tag, out);
            write(inner, out);
        }
        Value::Bool(false) => out.push(FALSE),
        Value::Bool(true) => out.push(TRUE),
        Value::Null => out.push(NULL),
        Value::Undefined => out.push(UNDEFINED),
        Value::Float(n) => write_float(*n, out),
    }
}

fn write_map(entries: &[(Value, Value)], out: &mut Vec<u8>) {
    let mut encoded: Vec<(Vec<u8>, &Value)> = entries
        .iter()
        .map(|(key, value)| {
            let mut bytes = Vec::new();
            write(key, &mut bytes);
            (bytes, value)
        })
        .collect();
    encoded.sort_by(|(left, _), (right, _)| canonical(left, right));

    head(MAP, entries.len() as u64, out);
    for (key, value) in encoded {
        out.extend_from_slice(&key);
        write(value, out);
    }
}

fn canonical(left: &[u8], right: &[u8]) -> Ordering {
    let major = |bytes: &[u8]| bytes.first().map_or(0, |first| first >> 5);
    major(left)
        .cmp(&major(right))
        .then(left.len().cmp(&right.len()))
        .then_with(|| left.cmp(right))
}

fn write_float(n: f64, out: &mut Vec<u8>) {
    let narrow = n as f32;
    if f64::from(narrow) == n || n.is_nan() {
        out.push(SINGLE);
        out.extend_from_slice(&narrow.to_be_bytes());
    } else {
        out.push(DOUBLE);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn head(major: u8, argument: u64, out: &mut Vec<u8>) {
    let major = major << 5;
    match argument {
        0..=23 => out.push(major | argument as u8),
        24..=0xff => out.extend_from_slice(&[major | 24, argument as u8]),
        0x100..=0xffff => {
            out.push(major | 25);
            out.extend_from_slice(&(argument as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(major | 26);
            out.extend_from_slice(&(argument as u32).to_be_bytes());
        }
        _ => {
            out.push(major | 27);
            out.extend_from_slice(&argument.to_be_bytes());
        }
    }
}
