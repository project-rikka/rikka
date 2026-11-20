use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;

use crate::document::{Node, Value};
use rikka_model_core::ByteWriter;

const HEAD: &[u8] = b"Kaydara FBX Binary  \x00\x1a\x00";
const FOOT: [u8; 16] = [
    0xfa, 0xbc, 0xab, 0x09, 0xd0, 0xc8, 0xd4, 0x66, 0xb1, 0x76, 0xfb, 0x83, 0x1c, 0xf7, 0x26, 0x7e,
];
const TAIL: [u8; 16] = [
    0xf8, 0x5a, 0x8c, 0x6a, 0xde, 0xf5, 0xd9, 0x7e, 0xec, 0xe9, 0x0c, 0xe3, 0x75, 0x8f, 0x29, 0x0b,
];
const WIDE_VERSION: u32 = 7500;
const PACK_ABOVE: usize = 128;

const ALWAYS_SENTINEL: [&str; 2] = ["AnimationStack", "AnimationLayer"];

pub(crate) fn write(nodes: &[Node], version: u32) -> Vec<u8> {
    let mut out = ByteWriter::default();
    out.bytes(HEAD);
    out.u32(version);

    let wide = version >= WIDE_VERSION;
    let mut writer = Writer { out, wide };
    writer.top_level(nodes);
    let mut out = writer.out;

    out.bytes(&FOOT);
    out.fill(0, 4);
    let padding = match out.len() % 16 {
        0 => 16,
        remainder => 16 - remainder,
    };
    out.fill(0, padding);
    out.u32(version);
    out.fill(0, 120);
    out.bytes(&TAIL);
    out.finish()
}

struct Writer {
    out: ByteWriter,
    wide: bool,
}

impl Writer {
    fn top_level(&mut self, nodes: &[Node]) {
        for (index, node) in nodes.iter().enumerate() {
            self.node(node, index + 1 == nodes.len());
        }
        self.sentinel();
    }

    fn node(&mut self, node: &Node, last: bool) {
        let header = self.out.len();
        self.offset(0);
        self.offset(node.values.len() as u64);
        self.offset(0);
        self.out.u8(node.name.len() as u8);
        self.out.bytes(node.name.as_bytes());

        let values = self.out.len();
        for value in &node.values {
            self.value(value);
        }
        let length = (self.out.len() - values) as u64;

        let sentinel = !node.children.is_empty()
            || ALWAYS_SENTINEL.contains(&node.name.as_str())
            || (node.values.is_empty() && !last);
        for (index, child) in node.children.iter().enumerate() {
            self.node(child, index + 1 == node.children.len());
        }
        if sentinel {
            self.sentinel();
        }

        let end = self.out.len() as u64;
        self.patch(header, end);
        self.patch(header + self.width() * 2, length);
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Bool(value) => self.scalar(b'C', &[u8::from(*value)]),
            Value::I16(value) => self.scalar(b'Y', &value.to_le_bytes()),
            Value::I32(value) => self.scalar(b'I', &value.to_le_bytes()),
            Value::I64(value) => self.scalar(b'L', &value.to_le_bytes()),
            Value::F32(value) => self.scalar(b'F', &value.to_le_bytes()),
            Value::F64(value) => self.scalar(b'D', &value.to_le_bytes()),
            Value::Text(text) | Value::Word(text) => self.bytes(b'S', text.as_bytes()),
            Value::Raw(bytes) => self.bytes(b'R', bytes),
            Value::Bools(values) => {
                self.array(
                    b'b',
                    values.len(),
                    values.iter().flat_map(|&value| [u8::from(value)]),
                );
            }
            Value::I32s(values) => {
                self.array(
                    b'i',
                    values.len(),
                    values.iter().flat_map(|value| value.to_le_bytes()),
                );
            }
            Value::I64s(values) => {
                self.array(
                    b'l',
                    values.len(),
                    values.iter().flat_map(|value| value.to_le_bytes()),
                );
            }
            Value::F32s(values) => {
                self.array(
                    b'f',
                    values.len(),
                    values.iter().flat_map(|value| value.to_le_bytes()),
                );
            }
            Value::F64s(values) => {
                self.array(
                    b'd',
                    values.len(),
                    values.iter().flat_map(|value| value.to_le_bytes()),
                );
            }
        }
    }

    fn scalar(&mut self, kind: u8, bytes: &[u8]) {
        self.out.u8(kind);
        self.out.bytes(bytes);
    }

    fn bytes(&mut self, kind: u8, bytes: &[u8]) {
        self.out.u8(kind);
        self.out.u32(bytes.len() as u32);
        self.out.bytes(bytes);
    }

    fn array(&mut self, kind: u8, count: usize, bytes: impl Iterator<Item = u8>) {
        let raw: Vec<u8> = bytes.collect();
        let packed = (raw.len() > PACK_ABOVE).then(|| deflate(&raw)).flatten();
        let (encoding, data) = match &packed {
            Some(packed) => (1u32, packed.as_slice()),
            None => (0u32, raw.as_slice()),
        };
        self.out.u8(kind);
        self.out.u32(count as u32);
        self.out.u32(encoding);
        self.out.u32(data.len() as u32);
        self.out.bytes(data);
    }

    fn sentinel(&mut self) {
        self.out.fill(0, self.width() * 3 + 1);
    }

    fn offset(&mut self, value: u64) {
        match self.wide {
            true => self.out.u64(value),
            false => self.out.u32(value as u32),
        }
    }

    fn patch(&mut self, at: usize, value: u64) {
        match self.wide {
            true => self.out.set_u64(at, value),
            false => self.out.set_u32(at, value as u32),
        }
    }

    fn width(&self) -> usize {
        match self.wide {
            true => 8,
            false => 4,
        }
    }
}

fn deflate(raw: &[u8]) -> Option<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(raw).ok()?;
    let packed = encoder.finish().ok()?;
    (packed.len() < raw.len()).then_some(packed)
}
