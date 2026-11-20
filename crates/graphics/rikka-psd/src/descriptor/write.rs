use super::value::{Class, Descriptor, Id, Reference, Value};
use crate::error::{Error, Result};
use crate::io::Writer;

pub(crate) fn write_versioned(writer: &mut Writer, descriptor: &Descriptor) -> Result<()> {
    writer.u32(16);
    write_descriptor(writer, descriptor)
}

pub(crate) fn write_descriptor(writer: &mut Writer, descriptor: &Descriptor) -> Result<()> {
    writer.unicode_terminated(&descriptor.name);
    id(writer, &descriptor.class)?;
    writer.u32(descriptor.items.len() as u32);
    for (key, item) in &descriptor.items {
        id(writer, key)?;
        value(writer, item)?;
    }
    Ok(())
}

fn id(writer: &mut Writer, id: &Id) -> Result<()> {
    match id {
        Id::Code(key) => {
            writer.u32(0);
            writer.key(*key);
        }
        Id::Text(text) => {
            writer.u32(u32::try_from(text.len()).map_err(|_| Error::Oversized)?);
            writer.bytes(text.as_bytes());
        }
    }
    Ok(())
}

fn class(writer: &mut Writer, class: &Class) -> Result<()> {
    writer.unicode_terminated(&class.name);
    id(writer, &class.id)
}

fn bytes(writer: &mut Writer, data: &[u8]) -> Result<()> {
    writer.u32(u32::try_from(data.len()).map_err(|_| Error::Oversized)?);
    writer.bytes(data);
    Ok(())
}

fn value(writer: &mut Writer, value: &Value) -> Result<()> {
    writer.code(type_code(value));
    payload(writer, value)
}

fn type_code(value: &Value) -> &'static [u8; 4] {
    match value {
        Value::Reference(_) => b"obj ",
        Value::Descriptor(_) => b"Objc",
        Value::GlobalObject(_) => b"GlbO",
        Value::List(_) => b"VlLs",
        Value::Double(_) => b"doub",
        Value::UnitDouble(..) => b"UntF",
        Value::UnitFloats(..) => b"UnFl",
        Value::String(_) => b"TEXT",
        Value::Enum(..) => b"enum",
        Value::Integer(_) => b"long",
        Value::LargeInteger(_) => b"comp",
        Value::Bool(_) => b"bool",
        Value::Class(_) => b"type",
        Value::GlobalClass(_) => b"GlbC",
        Value::Alias(_) => b"alis",
        Value::RawData(_) => b"tdta",
        Value::Path(_) => b"Pth ",
        Value::ObjectArray(_) => b"ObAr",
    }
}

fn payload(writer: &mut Writer, item: &Value) -> Result<()> {
    match item {
        Value::Reference(items) => {
            writer.u32(items.len() as u32);
            for item in items {
                reference(writer, item)?;
            }
        }
        Value::Descriptor(descriptor) | Value::GlobalObject(descriptor) => {
            write_descriptor(writer, descriptor)?
        }
        Value::List(items) => {
            writer.u32(items.len() as u32);
            for item in items {
                value(writer, item)?;
            }
        }
        Value::Double(number) => writer.f64(*number),
        Value::UnitDouble(unit, number) => {
            writer.key(unit.0);
            writer.f64(*number);
        }
        Value::UnitFloats(unit, numbers) => {
            writer.key(unit.0);
            writer.u32(numbers.len() as u32);
            for number in numbers {
                writer.f64(*number);
            }
        }
        Value::String(text) => writer.unicode_terminated(text),
        Value::Enum(kind, member) => {
            id(writer, kind)?;
            id(writer, member)?;
        }
        Value::Integer(number) => writer.i32(*number),
        Value::LargeInteger(number) => writer.i64(*number),
        Value::Bool(flag) => writer.bool(*flag),
        Value::Class(item) | Value::GlobalClass(item) => class(writer, item)?,
        Value::Alias(data) | Value::RawData(data) | Value::Path(data) => bytes(writer, data)?,
        Value::ObjectArray(array) => {
            writer.u32(array.count);
            writer.unicode_terminated(&array.name);
            id(writer, &array.class)?;
            writer.u32(array.items.len() as u32);
            for (key, item) in &array.items {
                id(writer, key)?;
                value(writer, item)?;
            }
        }
    }
    Ok(())
}

fn reference(writer: &mut Writer, item: &Reference) -> Result<()> {
    match item {
        Reference::Property { class: owner, key } => {
            writer.code(b"prop");
            class(writer, owner)?;
            id(writer, key)?;
        }
        Reference::Class(owner) => {
            writer.code(b"Clss");
            class(writer, owner)?;
        }
        Reference::Enumerated {
            class: owner,
            kind,
            value,
        } => {
            writer.code(b"Enmr");
            class(writer, owner)?;
            id(writer, kind)?;
            id(writer, value)?;
        }
        Reference::Offset {
            class: owner,
            value,
        } => {
            writer.code(b"rele");
            class(writer, owner)?;
            writer.u32(*value);
        }
        Reference::Identifier(value) => {
            writer.code(b"Idnt");
            writer.i32(*value);
        }
        Reference::Index(value) => {
            writer.code(b"indx");
            writer.i32(*value);
        }
        Reference::Name {
            class: owner,
            value,
        } => {
            writer.code(b"name");
            class(writer, owner)?;
            writer.unicode_terminated(value);
        }
    }
    Ok(())
}
