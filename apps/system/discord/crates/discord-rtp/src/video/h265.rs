use rikka_annexb::START_CODE;

use crate::video::aggregate::aggregated;

const HEADER_LEN: usize = 2;
const AGGREGATE: u8 = 48;
const FRAGMENT: u8 = 49;
const FRAGMENT_START: u8 = 0x80;
const RANDOM_ACCESS: std::ops::RangeInclusive<u8> = 16..=21;
const PARAMETER_SETS: std::ops::RangeInclusive<u8> = 32..=34;

fn kind(header: u8) -> u8 {
    (header >> 1) & 0x3F
}

pub(crate) fn is_keyframe(payload: &[u8]) -> bool {
    let Some(&first) = payload.first() else {
        return false;
    };
    let starts_picture = |kind: u8| RANDOM_ACCESS.contains(&kind) || PARAMETER_SETS.contains(&kind);
    match kind(first) {
        AGGREGATE => aggregated(payload.get(HEADER_LEN..).unwrap_or_default())
            .iter()
            .any(|unit| {
                unit.first()
                    .is_some_and(|&header| starts_picture(kind(header)))
            }),
        FRAGMENT => payload.get(HEADER_LEN).is_some_and(|&flags| {
            flags & FRAGMENT_START != 0 && RANDOM_ACCESS.contains(&(flags & 0x3F))
        }),
        other => starts_picture(other),
    }
}

pub(crate) fn assemble(payloads: &[&[u8]]) -> Option<Vec<u8>> {
    let mut frame = Vec::with_capacity(payloads.iter().map(|payload| payload.len() + 4).sum());
    for payload in payloads {
        if payload.len() < HEADER_LEN {
            return None;
        }
        match kind(payload[0]) {
            AGGREGATE => {
                let units = aggregated(&payload[HEADER_LEN..]);
                if units.is_empty() {
                    return None;
                }
                for unit in units {
                    frame.extend_from_slice(&START_CODE);
                    frame.extend_from_slice(unit);
                }
            }
            FRAGMENT => {
                let &flags = payload.get(HEADER_LEN)?;
                if flags & FRAGMENT_START != 0 {
                    frame.extend_from_slice(&START_CODE);
                    frame.push((payload[0] & 0x81) | ((flags & 0x3F) << 1));
                    frame.push(payload[1]);
                }
                frame.extend_from_slice(&payload[HEADER_LEN + 1..]);
            }
            50.. => return None,
            _ => {
                frame.extend_from_slice(&START_CODE);
                frame.extend_from_slice(payload);
            }
        }
    }
    Some(frame)
}
