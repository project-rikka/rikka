use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::details::Details;

use super::fault::{Fault, Probed};

const MAGIC: &[u8] = b"BLENDER";
const ZSTD: &[u8] = &[0x28, 0xb5, 0x2f, 0xfd];
const GZIP: &[u8] = &[0x1f, 0x8b];

pub(super) fn read(path: &Path) -> Probed<Details> {
    let mut head = [0; 7];
    File::open(path)?
        .read_exact(&mut head)
        .map_err(Fault::read)?;
    let known = [MAGIC, ZSTD, GZIP]
        .iter()
        .any(|magic| head.starts_with(magic));
    match known {
        true => Ok(Details::Model(Box::default())),
        false => Err(Fault::Foreign("Blend")),
    }
}
