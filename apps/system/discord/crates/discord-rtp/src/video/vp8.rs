const EXTENDED: u8 = 0x80;
const START: u8 = 0x10;
const PARTITION: u8 = 0x07;
const PICTURE_ID: u8 = 0x80;
const LAYER_INDEX: u8 = 0x40;
const TEMPORAL: u8 = 0x20;
const KEY_INDEX: u8 = 0x10;
const LONG_PICTURE_ID: u8 = 0x80;
const INTER: u8 = 0x01;

use crate::video::described::{self, Described};

pub(crate) fn describe(payload: &[u8]) -> Option<Described> {
    let &first = payload.first()?;
    let mut skip = 1;
    if first & EXTENDED != 0 {
        let &extension = payload.get(skip)?;
        skip += 1;
        if extension & PICTURE_ID != 0 {
            let &picture = payload.get(skip)?;
            skip += if picture & LONG_PICTURE_ID != 0 { 2 } else { 1 };
        }
        if extension & LAYER_INDEX != 0 {
            skip += 1;
        }
        if extension & (TEMPORAL | KEY_INDEX) != 0 {
            skip += 1;
        }
    }
    if skip >= payload.len() {
        return None;
    }
    let starts = first & START != 0 && first & PARTITION == 0;
    Some(Described {
        starts,
        keyframe: starts && payload[skip] & INTER == 0,
        skip,
    })
}

pub(crate) fn assemble(payloads: &[&[u8]]) -> Option<Vec<u8>> {
    described::assemble(payloads, describe)
}
