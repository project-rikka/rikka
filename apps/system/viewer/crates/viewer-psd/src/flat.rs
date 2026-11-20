use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use rikka_psd::{Document, FileHeader};
use rikka_psd_render::render_rgba;

use crate::error::{Error, Result};

const WORKING_MEMORY: u64 = 2 << 30;
const SHOWN_BYTES_PER_PIXEL: u64 = 4;
const RENDERED_BYTES_PER_PIXEL: u64 = 64;

#[derive(Clone, Debug)]
pub struct Flat {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
}

pub fn flatten(path: &Path) -> Result<Flat> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(FileHeader::LEN);
    file.by_ref()
        .take(FileHeader::LEN as u64)
        .read_to_end(&mut bytes)?;
    let header = FileHeader::read(&bytes)?;
    afford(&header, stored_cost(&header))?;

    file.read_to_end(&mut bytes)?;
    let document = Document::read(&bytes)?;
    drop(bytes);

    let pixels = match document.has_real_composite() {
        true => document.composite_rgba8()?,
        false => {
            afford(&header, header.pixel_count() * RENDERED_BYTES_PER_PIXEL)?;
            render_rgba(&document)?.to_rgba8()
        }
    };
    Ok(Flat {
        width: header.width,
        height: header.height,
        pixels: Arc::from(pixels),
    })
}

fn stored_cost(header: &FileHeader) -> u64 {
    header.pixel_count() * SHOWN_BYTES_PER_PIXEL + header.composite_bytes()
}

fn afford(header: &FileHeader, bytes: u64) -> Result<()> {
    match bytes <= WORKING_MEMORY {
        true => Ok(()),
        false => Err(Error::TooLarge {
            width: header.width,
            height: header.height,
        }),
    }
}
