const VARINT: u8 = 0;
const FIXED64: u8 = 1;
const LENGTH: u8 = 2;
const FIXED32: u8 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Payload {
    Varint(u64),
    Fixed64(u64),
    Length(Vec<u8>),
    Fixed32(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Field {
    tag: u32,
    payload: Payload,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Fields(Vec<Field>);

impl Fields {
    #[must_use]
    pub(crate) fn decode(mut bytes: &[u8]) -> Option<Fields> {
        let mut fields = Vec::new();
        while !bytes.is_empty() {
            let key = varint(&mut bytes)?;
            let tag = u32::try_from(key >> 3).ok()?;
            let payload = match (key & 7) as u8 {
                VARINT => Payload::Varint(varint(&mut bytes)?),
                FIXED64 => {
                    Payload::Fixed64(u64::from_le_bytes(take(&mut bytes, 8)?.try_into().ok()?))
                }
                LENGTH => {
                    let length = usize::try_from(varint(&mut bytes)?).ok()?;
                    Payload::Length(take(&mut bytes, length)?.to_vec())
                }
                FIXED32 => {
                    Payload::Fixed32(u32::from_le_bytes(take(&mut bytes, 4)?.try_into().ok()?))
                }
                _ => return None,
            };
            fields.push(Field { tag, payload });
        }
        Some(Fields(fields))
    }

    #[must_use]
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for field in &self.0 {
            let wire = match field.payload {
                Payload::Varint(_) => VARINT,
                Payload::Fixed64(_) => FIXED64,
                Payload::Length(_) => LENGTH,
                Payload::Fixed32(_) => FIXED32,
            };
            put_varint(&mut out, (u64::from(field.tag) << 3) | u64::from(wire));
            match &field.payload {
                Payload::Varint(value) => put_varint(&mut out, *value),
                Payload::Fixed64(value) => out.extend_from_slice(&value.to_le_bytes()),
                Payload::Length(bytes) => {
                    put_varint(&mut out, bytes.len() as u64);
                    out.extend_from_slice(bytes);
                }
                Payload::Fixed32(value) => out.extend_from_slice(&value.to_le_bytes()),
            }
        }
        out
    }

    #[must_use]
    pub(crate) fn fixed64s(&self, tag: u32) -> Vec<u64> {
        self.0
            .iter()
            .filter(|field| field.tag == tag)
            .flat_map(|field| match &field.payload {
                Payload::Fixed64(value) => vec![*value],
                Payload::Length(packed) => packed
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|chunk| u64::from_le_bytes(*chunk))
                    .collect(),
                Payload::Varint(_) | Payload::Fixed32(_) => Vec::new(),
            })
            .collect()
    }

    pub(crate) fn set_fixed64s(&mut self, tag: u32, values: &[u64]) {
        let at = self.0.iter().position(|field| field.tag == tag);
        self.0.retain(|field| field.tag != tag);
        if values.is_empty() {
            return;
        }
        let packed = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let field = Field {
            tag,
            payload: Payload::Length(packed),
        };
        match at {
            Some(at) => self.0.insert(at.min(self.0.len()), field),
            None => self.0.push(field),
        }
    }
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Option<&'a [u8]> {
    if bytes.len() < count {
        return None;
    }
    let (head, rest) = bytes.split_at(count);
    *bytes = rest;
    Some(head)
}

fn varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = bytes.split_first()?;
        *bytes = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}
