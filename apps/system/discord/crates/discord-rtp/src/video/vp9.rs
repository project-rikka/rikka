const PICTURE_ID: u8 = 0x80;
const INTER: u8 = 0x40;
const LAYERS: u8 = 0x20;
const FLEXIBLE: u8 = 0x10;
const BEGINS: u8 = 0x08;
const STRUCTURE: u8 = 0x02;
const LONG_PICTURE_ID: u8 = 0x80;
const MORE_DIFFERENCES: u8 = 0x01;
const MAX_DIFFERENCES: usize = 3;

use crate::video::described::{self, Described};

pub(crate) fn describe(payload: &[u8]) -> Option<Described> {
    let &first = payload.first()?;
    let mut at = 1;
    if first & PICTURE_ID != 0 {
        let &picture = payload.get(at)?;
        at += if picture & LONG_PICTURE_ID != 0 { 2 } else { 1 };
    }
    if first & LAYERS != 0 {
        at += 1;
        if first & FLEXIBLE == 0 {
            at += 1;
        }
    }
    if first & FLEXIBLE != 0 && first & INTER != 0 {
        for _ in 0..MAX_DIFFERENCES {
            let &difference = payload.get(at)?;
            at += 1;
            if difference & MORE_DIFFERENCES == 0 {
                break;
            }
        }
    }
    if first & STRUCTURE != 0 {
        at = structure(payload, at)?;
    }
    if at > payload.len() {
        return None;
    }
    let starts = first & BEGINS != 0;
    Some(Described {
        starts,
        keyframe: starts && first & INTER == 0,
        skip: at,
    })
}

fn structure(payload: &[u8], mut at: usize) -> Option<usize> {
    let &shape = payload.get(at)?;
    at += 1;
    let spatial = usize::from(shape >> 5) + 1;
    if shape & 0x10 != 0 {
        at += spatial * 4;
    }
    if shape & 0x08 != 0 {
        let &groups = payload.get(at)?;
        at += 1;
        for _ in 0..groups {
            let &group = payload.get(at)?;
            at += 1 + usize::from((group >> 2) & 0x03);
        }
    }
    Some(at)
}

pub(crate) fn assemble(payloads: &[&[u8]]) -> Option<Vec<u8>> {
    described::assemble(payloads, describe)
}
