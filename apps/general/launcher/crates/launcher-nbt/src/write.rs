use crate::error::{Error, Result};
use crate::mutf8;
use crate::tag::{Compound, Kind, Tag};

fn string(out: &mut Vec<u8>, text: &str) -> Result<()> {
    let bytes = mutf8::encode(text);
    let length = u16::try_from(bytes.len()).map_err(|_| Error::LongText(bytes.len()))?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(&bytes);
    Ok(())
}

fn length(out: &mut Vec<u8>, count: usize) {
    let count = i32::try_from(count).expect("NBT arrays hold fewer than 2^31 items");
    out.extend_from_slice(&count.to_be_bytes());
}

fn payload(out: &mut Vec<u8>, tag: &Tag) -> Result<()> {
    match tag {
        Tag::Byte(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Short(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Int(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Long(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Float(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::Double(value) => out.extend_from_slice(&value.to_be_bytes()),
        Tag::ByteArray(values) => {
            length(out, values.len());
            out.extend(values.iter().map(|value| value.to_be_bytes()[0]));
        }
        Tag::String(text) => string(out, text)?,
        Tag::List(list) => {
            out.push(list.kind().id());
            length(out, list.items().len());
            for item in list.items() {
                payload(out, item)?;
            }
        }
        Tag::Compound(compound) => entries(out, compound)?,
        Tag::IntArray(values) => {
            length(out, values.len());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
        Tag::LongArray(values) => {
            length(out, values.len());
            for value in values {
                out.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
    Ok(())
}

fn entries(out: &mut Vec<u8>, compound: &Compound) -> Result<()> {
    for (name, tag) in compound.iter() {
        out.push(tag.kind().id());
        string(out, name)?;
        payload(out, tag)?;
    }
    out.push(Kind::End.id());
    Ok(())
}

pub fn write(name: &str, root: &Compound) -> Result<Vec<u8>> {
    let mut out = vec![Kind::Compound.id()];
    string(&mut out, name)?;
    entries(&mut out, root)?;
    Ok(out)
}
