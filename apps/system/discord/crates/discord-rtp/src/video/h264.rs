use rikka_annexb::{START_CODE, units};

use crate::video::aggregate::{AGGREGATE_SIZE_LEN, aggregated};
use crate::video::sps;

const TYPE_MASK: u8 = 0x1F;
const IDR: u8 = 5;
const SPS: u8 = 7;
const AGGREGATE: u8 = 24;
const FRAGMENT: u8 = 28;
const FRAGMENT_START: u8 = 0x80;
const FRAGMENT_END: u8 = 0x40;

pub(crate) fn prepare(frame: &[u8]) -> Vec<u8> {
    let mut prepared = Vec::with_capacity(frame.len() + START_CODE.len());
    for unit in units(frame) {
        let rewritten = match unit.first().map(|header| header & TYPE_MASK) {
            Some(SPS) => sps::rewrite(unit),
            _ => None,
        };
        prepared.extend_from_slice(&START_CODE);
        prepared.extend_from_slice(rewritten.as_deref().unwrap_or(unit));
    }
    prepared
}

pub(crate) fn packetize(frame: &[u8], most: usize) -> Vec<Vec<u8>> {
    let units = units(frame);
    let mut payloads = Vec::new();
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if unit.len() > most {
            fragment(unit, most, &mut payloads);
            index += 1;
            continue;
        }
        let mut gathered = vec![unit];
        let mut size = 1 + AGGREGATE_SIZE_LEN + unit.len();
        while let Some(next) = units.get(index + gathered.len()) {
            let grown = size + AGGREGATE_SIZE_LEN + next.len();
            if grown > most {
                break;
            }
            gathered.push(next);
            size = grown;
        }
        index += gathered.len();
        if gathered.len() == 1 {
            payloads.push(unit.to_vec());
            continue;
        }
        let importance = gathered
            .iter()
            .map(|unit| unit[0] & 0x60)
            .max()
            .unwrap_or(0);
        let mut aggregate = Vec::with_capacity(size);
        aggregate.push(importance | AGGREGATE);
        for unit in gathered {
            aggregate.extend_from_slice(&(unit.len() as u16).to_be_bytes());
            aggregate.extend_from_slice(unit);
        }
        payloads.push(aggregate);
    }
    payloads
}

fn fragment(unit: &[u8], most: usize, payloads: &mut Vec<Vec<u8>>) {
    let header = unit[0];
    let body = &unit[1..];
    let room = most - 2;
    let pieces = body.len().div_ceil(room);
    let even = body.len().div_ceil(pieces);
    let indicator = (header & 0xE0) | FRAGMENT;
    for (index, piece) in body.chunks(even).enumerate() {
        let mut flags = header & TYPE_MASK;
        if index == 0 {
            flags |= FRAGMENT_START;
        }
        if index + 1 == pieces {
            flags |= FRAGMENT_END;
        }
        let mut payload = Vec::with_capacity(piece.len() + 2);
        payload.push(indicator);
        payload.push(flags);
        payload.extend_from_slice(piece);
        payloads.push(payload);
    }
}

pub(crate) fn is_keyframe(payload: &[u8]) -> bool {
    let Some(&first) = payload.first() else {
        return false;
    };
    match first & TYPE_MASK {
        IDR | SPS => true,
        AGGREGATE => aggregated(&payload[1..]).iter().any(|unit| {
            matches!(
                unit.first().map(|header| header & TYPE_MASK),
                Some(IDR | SPS)
            )
        }),
        FRAGMENT => payload
            .get(1)
            .is_some_and(|flags| flags & FRAGMENT_START != 0 && flags & TYPE_MASK == IDR),
        _ => false,
    }
}

pub(crate) fn assemble(payloads: &[&[u8]]) -> Option<Vec<u8>> {
    let mut frame = Vec::with_capacity(payloads.iter().map(|payload| payload.len() + 4).sum());
    for payload in payloads {
        let &first = payload.first()?;
        match first & TYPE_MASK {
            1..=23 => {
                frame.extend_from_slice(&START_CODE);
                frame.extend_from_slice(payload);
            }
            AGGREGATE => {
                let units = aggregated(&payload[1..]);
                if units.is_empty() {
                    return None;
                }
                for unit in units {
                    frame.extend_from_slice(&START_CODE);
                    frame.extend_from_slice(unit);
                }
            }
            FRAGMENT => {
                let &flags = payload.get(1)?;
                if flags & FRAGMENT_START != 0 {
                    frame.extend_from_slice(&START_CODE);
                    frame.push((first & 0xE0) | (flags & TYPE_MASK));
                }
                frame.extend_from_slice(&payload[2..]);
            }
            _ => return None,
        }
    }
    Some(frame)
}
