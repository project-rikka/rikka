use super::value::{Class, Descriptor, Id, ObjectArray, Reference, Unit, Value};
use crate::error::{Error, Result};
use crate::io::Reader;
use crate::key::Key;

const MAX_NESTING: u32 = 128;

pub(crate) fn read_versioned(reader: &mut Reader<'_>) -> Result<Descriptor> {
    let version = reader.u32()?;
    if version != 16 {
        return Err(Error::Invalid {
            what: "descriptor version",
            value: i64::from(version),
        });
    }
    read_descriptor(reader)
}

fn read_descriptor(reader: &mut Reader<'_>) -> Result<Descriptor> {
    descriptor(reader, 0)
}

fn id(reader: &mut Reader<'_>) -> Result<Id> {
    let length = reader.u32()?;
    if length == 0 {
        return Ok(Id::Code(reader.key()?));
    }
    if length as usize > reader.remaining() {
        return Err(reader.malformed("descriptor identifier"));
    }
    let bytes = reader.bytes(length as usize)?;
    Ok(Id::Text(String::from_utf8_lossy(bytes).into_owned()))
}

fn class(reader: &mut Reader<'_>) -> Result<Class> {
    let name = reader.unicode()?;
    Ok(Class {
        name,
        id: id(reader)?,
    })
}

fn descriptor(reader: &mut Reader<'_>, depth: u32) -> Result<Descriptor> {
    if depth > MAX_NESTING {
        return Err(reader.malformed("descriptor nesting"));
    }
    let name = reader.unicode()?;
    let class = id(reader)?;
    let count = reader.u32()?;
    let mut items = Vec::with_capacity(count.min(1024) as usize);
    for _ in 0..count {
        let key = id(reader)?;
        let kind = reader.key()?;
        items.push((key, value(reader, kind, depth + 1)?));
    }
    Ok(Descriptor { name, class, items })
}

fn length_prefixed(reader: &mut Reader<'_>) -> Result<Vec<u8>> {
    let length = reader.u32()? as usize;
    Ok(reader.bytes(length)?.to_vec())
}

fn value(reader: &mut Reader<'_>, kind: Key, depth: u32) -> Result<Value> {
    if depth > MAX_NESTING {
        return Err(reader.malformed("descriptor nesting"));
    }
    Ok(match &kind.0 {
        b"obj " => {
            let count = reader.u32()?;
            let mut items = Vec::with_capacity(count.min(64) as usize);
            for _ in 0..count {
                items.push(reference(reader)?);
            }
            Value::Reference(items)
        }
        b"Objc" => Value::Descriptor(descriptor(reader, depth)?),
        b"GlbO" => Value::GlobalObject(descriptor(reader, depth)?),
        b"VlLs" => {
            let count = reader.u32()?;
            let mut items = Vec::with_capacity(count.min(1024) as usize);
            for _ in 0..count {
                let kind = reader.key()?;
                items.push(value(reader, kind, depth + 1)?);
            }
            Value::List(items)
        }
        b"doub" => Value::Double(reader.f64()?),
        b"UntF" => {
            let unit = Unit(reader.key()?);
            Value::UnitDouble(unit, reader.f64()?)
        }
        b"UnFl" => {
            let unit = Unit(reader.key()?);
            let count = reader.u32()? as usize;
            if count * 8 > reader.remaining() {
                return Err(reader.malformed("unit floats"));
            }
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(reader.f64()?);
            }
            Value::UnitFloats(unit, values)
        }
        b"TEXT" => Value::String(reader.unicode()?),
        b"enum" => {
            let kind = id(reader)?;
            Value::Enum(kind, id(reader)?)
        }
        b"long" => Value::Integer(reader.i32()?),
        b"comp" => Value::LargeInteger(reader.i64()?),
        b"bool" => Value::Bool(reader.bool()?),
        b"type" => Value::Class(class(reader)?),
        b"GlbC" => Value::GlobalClass(class(reader)?),
        b"alis" => Value::Alias(length_prefixed(reader)?),
        b"tdta" => Value::RawData(length_prefixed(reader)?),
        b"Pth " => Value::Path(length_prefixed(reader)?),
        b"ObAr" => {
            let count = reader.u32()?;
            let inner = descriptor(reader, depth)?;
            Value::ObjectArray(ObjectArray {
                count,
                name: inner.name,
                class: inner.class,
                items: inner.items,
            })
        }
        _ => return Err(Error::DescriptorType(kind)),
    })
}

fn reference(reader: &mut Reader<'_>) -> Result<Reference> {
    let kind = reader.key()?;
    Ok(match &kind.0 {
        b"prop" => {
            let class = class(reader)?;
            Reference::Property {
                class,
                key: id(reader)?,
            }
        }
        b"Clss" => Reference::Class(class(reader)?),
        b"Enmr" => {
            let class = class(reader)?;
            let kind = id(reader)?;
            Reference::Enumerated {
                class,
                kind,
                value: id(reader)?,
            }
        }
        b"rele" => {
            let class = class(reader)?;
            Reference::Offset {
                class,
                value: reader.u32()?,
            }
        }
        b"Idnt" => Reference::Identifier(reader.i32()?),
        b"indx" => Reference::Index(reader.i32()?),
        b"name" => {
            let class = class(reader)?;
            Reference::Name {
                class,
                value: reader.unicode()?,
            }
        }
        _ => return Err(Error::DescriptorType(kind)),
    })
}
