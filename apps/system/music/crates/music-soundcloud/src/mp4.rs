const HEADER: usize = 8;
const WIDE_HEADER: usize = 16;
const MEDIA_HEADER: [&[u8; 4]; 4] = [b"moov", b"trak", b"mdia", b"mdhd"];
const SEGMENT_INDEX: [&[u8; 4]; 1] = [b"sidx"];
const REFERENCE: usize = 12;

pub(crate) fn with_duration(mut head: Vec<u8>, seconds: f64) -> Vec<u8> {
    if let Some((start, end)) = located(&head, &MEDIA_HEADER) {
        stamp_media(&mut head[start..end], seconds);
    }
    if let Some((start, end)) = located(&head, &SEGMENT_INDEX) {
        stamp_index(&mut head[start..end], seconds);
    }
    head
}

fn located(bytes: &[u8], path: &[&[u8; 4]]) -> Option<(usize, usize)> {
    path.iter()
        .try_fold((0, bytes.len()), |(start, end), kind| {
            boxed(&bytes[start..end], kind).map(|(from, to)| (start + from, start + to))
        })
}

fn boxed(within: &[u8], kind: &[u8; 4]) -> Option<(usize, usize)> {
    let mut at = 0;
    while at + HEADER <= within.len() {
        let (header, length) = match read_u32(within, at)? {
            0 => (HEADER, within.len() - at),
            1 => (
                WIDE_HEADER,
                usize::try_from(read_u64(within, at + HEADER)?).ok()?,
            ),
            size => (HEADER, usize::try_from(size).ok()?),
        };
        let end = at
            .checked_add(length)
            .filter(|end| *end <= within.len() && length >= header)?;
        if &within[at + 4..at + HEADER] == kind {
            return Some((at + header, end));
        }
        at = end;
    }
    None
}

fn stamp_media(mdhd: &mut [u8], seconds: f64) {
    let (scale_at, duration_at, wide) = match mdhd.first() {
        Some(0) => (12, 16, false),
        Some(1) => (20, 24, true),
        _ => return,
    };
    let Some(scale) = read_u32(mdhd, scale_at) else {
        return;
    };
    let units = units(seconds, scale);
    match wide {
        true => write(mdhd, duration_at, &units.to_be_bytes()),
        false => write(mdhd, duration_at, &narrow(units).to_be_bytes()),
    }
}

fn stamp_index(sidx: &mut [u8], seconds: f64) {
    let (count_at, first_at) = match sidx.first() {
        Some(0) => (22, 24),
        Some(1) => (30, 32),
        _ => return,
    };
    let (Some(scale), Some(count)) = (
        read_u32(sidx, 8),
        sidx.get(count_at..count_at + 2)
            .map(|count| usize::from(u16::from_be_bytes([count[0], count[1]]))),
    ) else {
        return;
    };
    let Some(others) = (1..count)
        .map(|reference| read_u32(sidx, first_at + reference * REFERENCE + 4).map(u64::from))
        .sum::<Option<u64>>()
    else {
        return;
    };
    let first = units(seconds, scale).saturating_sub(others);
    write(sidx, first_at + 4, &narrow(first).to_be_bytes());
}

fn units(seconds: f64, scale: u32) -> u64 {
    (seconds.max(0.0) * f64::from(scale)).round() as u64
}

fn narrow(units: u64) -> u32 {
    u32::try_from(units).unwrap_or(u32::MAX - 1)
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn write(bytes: &mut [u8], at: usize, value: &[u8]) {
    if let Some(field) = bytes.get_mut(at..at + value.len()) {
        field.copy_from_slice(value);
    }
}
