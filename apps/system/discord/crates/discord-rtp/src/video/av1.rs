const CONTINUES: u8 = 0x80;
const CONTINUED: u8 = 0x40;
const COUNT: u8 = 0x30;
const NEW_SEQUENCE: u8 = 0x08;
const HAS_EXTENSION: u8 = 0x04;
const HAS_SIZE: u8 = 0x02;

pub(crate) fn starts(payload: &[u8]) -> bool {
    payload.first().is_some_and(|first| first & CONTINUES == 0)
}

pub(crate) fn is_keyframe(payload: &[u8]) -> bool {
    payload
        .first()
        .is_some_and(|first| first & NEW_SEQUENCE != 0 && first & CONTINUES == 0)
}

pub(crate) fn assemble(payloads: &[&[u8]]) -> Option<Vec<u8>> {
    let units = units(payloads)?;
    let mut frame = Vec::new();
    for unit in units {
        write_unit(&unit, &mut frame)?;
    }
    Some(frame)
}

fn units(payloads: &[&[u8]]) -> Option<Vec<Vec<u8>>> {
    let mut units: Vec<Vec<u8>> = Vec::new();
    let mut expect_continuation = false;
    for payload in payloads {
        let (&aggregation, mut rest) = payload.split_first()?;
        let continues = aggregation & CONTINUES != 0;
        if continues != expect_continuation {
            return None;
        }
        let expected = usize::from((aggregation & COUNT) >> 4);
        if rest.is_empty() {
            if expected != 1 {
                return None;
            }
            if !continues {
                units.push(Vec::new());
            }
            expect_continuation = aggregation & CONTINUED != 0;
            continue;
        }
        let mut index = 1;
        while !rest.is_empty() {
            if !(index == 1 && continues) {
                units.push(Vec::new());
            }
            let size = if index == expected {
                rest.len()
            } else {
                let (size, read) = leb128(rest)?;
                rest = &rest[read..];
                usize::try_from(size)
                    .ok()
                    .filter(|size| *size <= rest.len())?
            };
            units.last_mut()?.extend_from_slice(&rest[..size]);
            rest = &rest[size..];
            index += 1;
        }
        expect_continuation = aggregation & CONTINUED != 0;
    }
    (!expect_continuation).then_some(units)
}

fn write_unit(unit: &[u8], frame: &mut Vec<u8>) -> Option<()> {
    let (&header, mut rest) = unit.split_first()?;
    frame.push(header | HAS_SIZE);
    if header & HAS_EXTENSION != 0 {
        let (&extension, after) = rest.split_first()?;
        frame.push(extension);
        rest = after;
    }
    if header & HAS_SIZE != 0 {
        let (size, read) = leb128(rest)?;
        rest = &rest[read..];
        if usize::try_from(size).ok()? != rest.len() {
            return None;
        }
    }
    write_leb128(rest.len() as u64, frame);
    frame.extend_from_slice(rest);
    Some(())
}

fn leb128(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (index, &byte) in bytes.iter().enumerate().take(8) {
        value |= u64::from(byte & 0x7F) << (index * 7);
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

fn write_leb128(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}
