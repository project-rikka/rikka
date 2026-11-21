use rustc_hash::FxHashMap;

use rikka_model_core::{ByteReader, Endian, Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scalar {
    Signed,
    Unsigned,
    Float,
    Other,
}

#[derive(Debug)]
pub(crate) struct Kind {
    pub(crate) name: String,
    pub(crate) size: usize,
    pub(crate) scalar: Scalar,
}

#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) kind: usize,
    pub(crate) offset: usize,
    pub(crate) size: usize,
    pub(crate) count: usize,
    pub(crate) pointer: bool,
}

#[derive(Debug)]
pub(crate) struct Layout {
    pub(crate) name: String,
    pub(crate) size: usize,
    fields: FxHashMap<String, Field>,
}

impl Layout {
    pub(crate) fn field(&self, name: &str) -> Option<&Field> {
        self.fields.get(name)
    }

    pub(crate) fn field_count(&self) -> usize {
        self.fields.len()
    }
}

#[derive(Debug)]
pub(crate) struct Sdna {
    pub(crate) kinds: Vec<Kind>,
    pub(crate) layouts: Vec<Layout>,
    by_name: FxHashMap<String, usize>,
    kind_layout: FxHashMap<usize, usize>,
}

impl Sdna {
    pub(crate) fn parse(bytes: &[u8], endian: Endian, pointer_size: usize) -> Result<Self> {
        let mut reader = ByteReader::with_endian(bytes, endian);
        tag(&mut reader, b"SDNA")?;
        tag(&mut reader, b"NAME")?;
        let names = strings(&mut reader)?;
        reader.align(4)?;
        tag(&mut reader, b"TYPE")?;
        let type_names = strings(&mut reader)?;
        reader.align(4)?;
        tag(&mut reader, b"TLEN")?;
        let mut sizes = Vec::with_capacity(type_names.len());
        for _ in 0..type_names.len() {
            sizes.push(reader.u16()? as usize);
        }
        reader.align(4)?;
        tag(&mut reader, b"STRC")?;
        let count = count(&mut reader)?;

        let kinds: Vec<Kind> = type_names
            .into_iter()
            .zip(&sizes)
            .map(|(name, &size)| {
                let name = static_name(&name).to_owned();
                Kind {
                    scalar: scalar(&name),
                    name,
                    size,
                }
            })
            .collect();

        let mut layouts = Vec::with_capacity(count);
        let mut by_name = FxHashMap::default();
        let mut kind_layout = FxHashMap::default();
        for index in 0..count {
            let kind = reader.u16()? as usize;
            let members = reader.u16()? as usize;
            let owner = kinds.get(kind).ok_or_else(damaged)?;
            let mut fields: FxHashMap<String, Field> = FxHashMap::default();
            let mut offset = 0;
            let mut previous: Option<String> = None;
            for _ in 0..members {
                let member_kind = reader.u16()? as usize;
                let member_name = reader.u16()? as usize;
                let declared = names.get(member_name).ok_or_else(damaged)?;
                let member = kinds.get(member_kind).ok_or_else(damaged)?;
                let pointer = declared.starts_with('*') || declared.starts_with('(');
                let count = array_length(declared);
                let size = match pointer {
                    true => pointer_size * count,
                    false => member.size * count,
                };
                let mut name = bare(declared).to_owned();
                if name.is_empty()
                    && let Some(split) = previous
                        .as_ref()
                        .and_then(|previous_name| fields.remove(previous_name))
                {
                    offset = split.offset;
                    name = previous.clone().unwrap_or_default();
                }
                fields.entry(name.clone()).or_insert(Field {
                    kind: member_kind,
                    offset,
                    size,
                    count,
                    pointer,
                });
                previous = Some(name);
                offset += size;
            }
            let sound = offset == owner.size || owner.size == 0;
            by_name.entry(owner.name.clone()).or_insert(index);
            kind_layout.insert(kind, index);
            layouts.push(Layout {
                name: owner.name.clone(),
                size: owner.size,
                fields: match sound {
                    true => fields,
                    false => FxHashMap::default(),
                },
            });
        }
        Ok(Self {
            kinds,
            layouts,
            by_name,
            kind_layout,
        })
    }

    pub(crate) fn layout(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub(crate) fn layout_of_kind(&self, kind: usize) -> Option<usize> {
        self.kind_layout.get(&kind).copied()
    }
}

fn tag(reader: &mut ByteReader<'_>, expected: &[u8; 4]) -> Result<()> {
    match reader.take(4)? == expected {
        true => Ok(()),
        false => Err(Error::Damaged(format!(
            "SDNA is missing the {} section",
            String::from_utf8_lossy(expected)
        ))),
    }
}

fn count(reader: &mut ByteReader<'_>) -> Result<usize> {
    let count = reader.u32()? as usize;
    match count <= reader.remaining() {
        true => Ok(count),
        false => Err(damaged()),
    }
}

fn strings(reader: &mut ByteReader<'_>) -> Result<Vec<String>> {
    let count = count(reader)?;
    let mut strings = Vec::with_capacity(count);
    for _ in 0..count {
        strings.push(String::from_utf8_lossy(reader.c_string()?).into_owned());
    }
    Ok(strings)
}

fn damaged() -> Error {
    Error::Damaged("truncated or inconsistent SDNA".to_owned())
}

fn bare(declared: &str) -> &str {
    let start = declared
        .find(|character: char| character.is_ascii_alphanumeric() || character == '_')
        .unwrap_or(declared.len());
    let rest = &declared[start..];
    let end = rest
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(rest.len());
    &rest[..end]
}

fn array_length(declared: &str) -> usize {
    if declared.starts_with('(') {
        return 1;
    }
    let mut length = 1;
    let mut current = 0;
    let mut inside = false;
    for character in declared.chars() {
        match character {
            '[' => {
                inside = true;
                current = 0;
            }
            ']' if inside => {
                length *= current;
                inside = false;
            }
            digit if inside && digit.is_ascii_digit() => {
                current =
                    current * 10 + digit.to_digit(10).expect("tokens start with a digit") as usize;
            }
            _ => {}
        }
    }
    length
}

pub(crate) fn static_name(name: &str) -> &str {
    match name {
        "bScreen" => "Screen",
        "Collection" => "Group",
        "CollectionObject" => "GroupObject",
        other => other,
    }
}

fn scalar(name: &str) -> Scalar {
    match name {
        "char" | "short" | "int" | "long" | "int8_t" | "int16_t" | "int32_t" | "int64_t"
        | "bool" => Scalar::Signed,
        "uchar" | "ushort" | "ulong" | "uint" | "uint8_t" | "uint16_t" | "uint32_t"
        | "uint64_t" => Scalar::Unsigned,
        "float" | "double" => Scalar::Float,
        _ => Scalar::Other,
    }
}
