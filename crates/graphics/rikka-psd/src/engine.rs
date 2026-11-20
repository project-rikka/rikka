use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum EngineValue {
    Dict(Vec<(String, EngineValue)>),
    Array(Vec<EngineValue>),
    Integer(i64),
    Real(f64),
    Bool(bool),
    String(String),
    Name(String),
    Null,
}

impl EngineValue {
    pub fn get(&self, key: &str) -> Option<&EngineValue> {
        match self {
            EngineValue::Dict(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut EngineValue> {
        match self {
            EngineValue::Dict(entries) => entries
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn path(&self, keys: &[&str]) -> Option<&EngineValue> {
        keys.iter().try_fold(self, |value, key| value.get(key))
    }

    pub fn path_mut(&mut self, keys: &[&str]) -> Option<&mut EngineValue> {
        keys.iter().try_fold(self, |value, key| value.get_mut(key))
    }

    pub fn set(&mut self, key: &str, value: EngineValue) {
        if let EngineValue::Dict(entries) = self {
            match entries.iter_mut().find(|(name, _)| name == key) {
                Some(slot) => slot.1 = value,
                None => entries.push((key.to_owned(), value)),
            }
        }
    }

    pub fn number(&self) -> Option<f64> {
        match self {
            EngineValue::Integer(value) => Some(*value as f64),
            EngineValue::Real(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            EngineValue::String(text) | EngineValue::Name(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            EngineValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[EngineValue]> {
        match self {
            EngineValue::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<EngineValue>> {
        match self {
            EngineValue::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<EngineValue> {
        let mut parser = Parser {
            bytes,
            pos: 0,
            depth: 0,
        };
        parser.skip_space();
        let mut entries = Vec::new();
        if parser.peek_pair(b"<<") {
            let root = parser.value()?;
            parser.skip_space();
            return Ok(root);
        }
        while parser.pos < parser.bytes.len() {
            if parser.bytes[parser.pos] == 0 {
                break;
            }
            let key = parser.name()?;
            let value = parser.value()?;
            entries.push((key, value));
            parser.skip_space();
        }
        Ok(EngineValue::Dict(entries))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = b"\n\n".to_vec();
        write_value(&mut out, self, 0, false);
        out
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: u32,
}

impl Parser<'_> {
    fn error(&self) -> Error {
        Error::Malformed {
            what: "text engine data",
            offset: self.pos as u64,
        }
    }

    fn skip_space(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\r' | b'\n')
        {
            self.pos += 1;
        }
    }

    fn peek_pair(&self, pair: &[u8; 2]) -> bool {
        self.bytes.get(self.pos..self.pos + 2) == Some(pair.as_slice())
    }

    fn word(&mut self) -> &[u8] {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && !matches!(
                self.bytes[self.pos],
                b' ' | b'\t' | b'\r' | b'\n' | b'[' | b']' | b'(' | b'<' | b'>' | b'/'
            )
        {
            self.pos += 1;
        }
        &self.bytes[start..self.pos]
    }

    fn name(&mut self) -> Result<String> {
        self.skip_space();
        if self.bytes.get(self.pos) != Some(&b'/') {
            return Err(self.error());
        }
        self.pos += 1;
        Ok(String::from_utf8_lossy(self.word()).into_owned())
    }

    fn value(&mut self) -> Result<EngineValue> {
        self.skip_space();
        if self.depth > 256 {
            return Err(self.error());
        }
        let Some(&byte) = self.bytes.get(self.pos) else {
            return Err(self.error());
        };
        if self.peek_pair(b"<<") {
            self.pos += 2;
            self.depth += 1;
            let mut entries = Vec::new();
            loop {
                self.skip_space();
                if self.peek_pair(b">>") {
                    self.pos += 2;
                    break;
                }
                if self.pos >= self.bytes.len() {
                    return Err(self.error());
                }
                let key = self.name()?;
                let value = self.value()?;
                entries.push((key, value));
            }
            self.depth -= 1;
            return Ok(EngineValue::Dict(entries));
        }
        match byte {
            b'[' => {
                self.pos += 1;
                self.depth += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    match self.bytes.get(self.pos) {
                        Some(b']') => {
                            self.pos += 1;
                            break;
                        }
                        Some(_) => items.push(self.value()?),
                        None => return Err(self.error()),
                    }
                }
                self.depth -= 1;
                Ok(EngineValue::Array(items))
            }
            b'(' => {
                self.pos += 1;
                self.string()
            }
            b'/' => Ok(EngineValue::Name(self.name()?)),
            _ => {
                let word = self.word().to_vec();
                match word.as_slice() {
                    b"true" => Ok(EngineValue::Bool(true)),
                    b"false" => Ok(EngineValue::Bool(false)),
                    b"null" => Ok(EngineValue::Null),
                    _ => {
                        let text = std::str::from_utf8(&word).map_err(|_| self.error())?;
                        if text.contains('.') {
                            text.parse()
                                .map(EngineValue::Real)
                                .map_err(|_| self.error())
                        } else {
                            text.parse()
                                .map(EngineValue::Integer)
                                .map_err(|_| self.error())
                        }
                    }
                }
            }
        }
    }

    fn string(&mut self) -> Result<EngineValue> {
        let mut raw = Vec::new();
        loop {
            let Some(&byte) = self.bytes.get(self.pos) else {
                return Err(self.error());
            };
            self.pos += 1;
            match byte {
                b'\\' => {
                    let Some(&next) = self.bytes.get(self.pos) else {
                        return Err(self.error());
                    };
                    self.pos += 1;
                    raw.push(next);
                }
                b')' => break,
                other => raw.push(other),
            }
        }
        if raw.len() >= 2 && raw[..2] == [0xfe, 0xff] {
            let units: Vec<u16> = raw[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect();
            Ok(EngineValue::String(String::from_utf16_lossy(&units)))
        } else {
            Ok(EngineValue::String(crate::io::mac_roman::decode(&raw)))
        }
    }
}

fn indent(out: &mut Vec<u8>, depth: usize) {
    out.resize(out.len() + depth, b'\t');
}

fn real(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.1}");
    }
    let text = format!("{value:.5}");
    let text = text.trim_end_matches('0');
    let text = text
        .strip_suffix('.')
        .map_or_else(|| text.to_owned(), |stripped| format!("{stripped}.0"));
    if let Some(rest) = text.strip_prefix("0.") {
        format!(".{rest}")
    } else if let Some(rest) = text.strip_prefix("-0.") {
        format!("-.{rest}")
    } else {
        text
    }
}

fn write_string(out: &mut Vec<u8>, text: &str) {
    out.push(b'(');
    out.extend_from_slice(&[0xfe, 0xff]);
    for unit in text.encode_utf16() {
        for byte in unit.to_be_bytes() {
            if matches!(byte, b'(' | b')' | b'\\') {
                out.push(b'\\');
            }
            out.push(byte);
        }
    }
    out.push(b')');
}

fn write_value(out: &mut Vec<u8>, value: &EngineValue, depth: usize, in_property: bool) {
    match value {
        EngineValue::Dict(entries) => {
            if in_property {
                out.push(b'\n');
            }
            indent(out, depth);
            out.extend_from_slice(b"<<\n");
            for (key, item) in entries {
                indent(out, depth + 1);
                out.push(b'/');
                out.extend_from_slice(key.as_bytes());
                write_value(out, item, depth + 1, true);
                out.push(b'\n');
            }
            indent(out, depth);
            out.extend_from_slice(b">>");
        }
        EngineValue::Array(items) => {
            prefix(out, depth, in_property);
            if items
                .iter()
                .all(|item| matches!(item, EngineValue::Integer(_) | EngineValue::Real(_)))
            {
                out.push(b'[');
                for item in items {
                    out.push(b' ');
                    out.extend_from_slice(scalar(item).as_bytes());
                }
                out.extend_from_slice(b" ]");
            } else {
                out.extend_from_slice(b"[\n");
                for item in items {
                    write_value(out, item, depth + 1, false);
                    out.push(b'\n');
                }
                indent(out, depth);
                out.push(b']');
            }
        }
        EngineValue::String(text) => {
            prefix(out, depth, in_property);
            write_string(out, text);
        }
        other => {
            prefix(out, depth, in_property);
            out.extend_from_slice(scalar(other).as_bytes());
        }
    }
}

fn prefix(out: &mut Vec<u8>, depth: usize, in_property: bool) {
    if in_property {
        out.push(b' ');
    } else {
        indent(out, depth);
    }
}

fn scalar(value: &EngineValue) -> String {
    match value {
        EngineValue::Integer(number) => number.to_string(),
        EngineValue::Real(number) => real(*number),
        EngineValue::Bool(flag) => flag.to_string(),
        EngineValue::Name(name) => format!("/{name}"),
        EngineValue::Null => "null".to_owned(),
        EngineValue::String(text) => text.clone(),
        EngineValue::Dict(_) | EngineValue::Array(_) => String::new(),
    }
}
