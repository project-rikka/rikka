use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::details::{Details, ModelDetails};

use super::fault::{Fault, Probed, clean};

const MAGIC: &[u8; 4] = b"PMX ";
const LONGEST_TEXT: usize = 1 << 20;
const UTF16: u8 = 0;
const UTF8: u8 = 1;
const FEWEST_GLOBALS: u8 = 8;

pub(super) fn read(path: &Path) -> Probed<Details> {
    let mut input = BufReader::new(File::open(path)?);
    let mut magic = [0; 4];
    input.read_exact(&mut magic).map_err(Fault::read)?;
    if &magic != MAGIC {
        return Err(Fault::Foreign("PMX"));
    }
    let version = f32::from_le_bytes(array(&mut input)?);
    if !(1.9..=2.2).contains(&version) {
        return Err(Fault::Malformed("PMX version"));
    }
    let [count] = array(&mut input)?;
    if count < FEWEST_GLOBALS {
        return Err(Fault::Malformed("PMX header"));
    }
    let mut globals = vec![0; usize::from(count)];
    input.read_exact(&mut globals).map_err(Fault::read)?;
    let encoding = globals[0];
    if encoding != UTF16 && encoding != UTF8 {
        return Err(Fault::Malformed("PMX text encoding"));
    }
    let name = text(&mut input, encoding)?;
    let name_universal = text(&mut input, encoding)?;
    let comment = text(&mut input, encoding)?;
    let comment_universal = text(&mut input, encoding)?;
    let vertices = i32::from_le_bytes(array(&mut input)?);
    let vertices = u64::try_from(vertices).map_err(|_| Fault::Malformed("PMX vertex count"))?;
    Ok(Details::Model(Box::new(ModelDetails {
        title: name.or(name_universal),
        comment: comment.or(comment_universal),
        vertices: Some(vertices),
        ..ModelDetails::default()
    })))
}

fn array<const N: usize>(input: &mut impl Read) -> Probed<[u8; N]> {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes).map_err(Fault::read)?;
    Ok(bytes)
}

fn text(input: &mut impl Read, encoding: u8) -> Probed<Option<String>> {
    let length = i32::from_le_bytes(array(input)?);
    let length = usize::try_from(length).map_err(|_| Fault::Malformed("PMX text"))?;
    if length > LONGEST_TEXT {
        return Err(Fault::Malformed("PMX text"));
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes).map_err(Fault::read)?;
    let text = match encoding {
        UTF16 => {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(&bytes).into_owned(),
    };
    Ok(clean(&text))
}
