use crate::extent::Extent;

pub(crate) fn png(header: &[u8]) -> Option<Extent> {
    Extent::new(be32(header, 16)?, be32(header, 20)?)
}

pub(crate) fn jpeg(header: &[u8]) -> Option<Extent> {
    let mut at = 2;
    loop {
        if *header.get(at)? != 0xFF {
            return None;
        }
        let marker = loop {
            at += 1;
            let byte = *header.get(at)?;
            if byte != 0xFF {
                break byte;
            }
        };
        at += 1;
        match marker {
            0x01 | 0xD0..=0xD9 => continue,
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                return Extent::new(be16(header, at + 5)?, be16(header, at + 3)?);
            }
            _ => at += usize::try_from(be16(header, at)?).ok()?,
        }
    }
}

pub(crate) fn gif(header: &[u8]) -> Option<Extent> {
    Extent::new(le16(header, 6)?, le16(header, 8)?)
}

pub(crate) fn webp(header: &[u8]) -> Option<Extent> {
    match header.get(12..16)? {
        b"VP8 " => Extent::new(le16(header, 26)? & 0x3FFF, le16(header, 28)? & 0x3FFF),
        b"VP8L" => {
            let bits = le32(header, 21)?;
            Extent::new((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1)
        }
        b"VP8X" => Extent::new(le24(header, 24)? + 1, le24(header, 27)? + 1),
        _ => None,
    }
}

pub(crate) fn bmp(header: &[u8]) -> Option<Extent> {
    let width = le32(header, 18)?.cast_signed().unsigned_abs();
    let height = le32(header, 22)?.cast_signed().unsigned_abs();
    Extent::new(width, height)
}

pub(crate) fn ico(header: &[u8]) -> Option<Extent> {
    let side = |at: usize| match *header.get(at)? {
        0 => Some(256),
        side => Some(u32::from(side)),
    };
    Extent::new(side(6)?, side(7)?)
}

pub(crate) fn tiff(header: &[u8]) -> Option<Extent> {
    let big_endian = header.get(0..2)? == b"MM";
    let read16 = |at: usize| {
        if big_endian {
            be16(header, at)
        } else {
            le16(header, at)
        }
    };
    let read32 = |at: usize| {
        if big_endian {
            be32(header, at)
        } else {
            le32(header, at)
        }
    };

    let directory = usize::try_from(read32(4)?).ok()?;
    let entries = read16(directory)?;
    let mut width = None;
    let mut height = None;
    for index in 0..entries {
        let entry = directory + 2 + usize::try_from(index).ok()? * 12;
        let value = match read16(entry + 2)? {
            3 => read16(entry + 8)?,
            4 => read32(entry + 8)?,
            _ => continue,
        };
        match read16(entry)? {
            256 => width = Some(value),
            257 => height = Some(value),
            _ => {}
        }
    }
    Extent::new(width?, height?)
}

pub(crate) fn tga(header: &[u8]) -> Option<Extent> {
    Extent::new(le16(header, 12)?, le16(header, 14)?)
}

pub(crate) fn qoi(header: &[u8]) -> Option<Extent> {
    Extent::new(be32(header, 4)?, be32(header, 8)?)
}

pub(crate) fn dds(header: &[u8]) -> Option<Extent> {
    Extent::new(le32(header, 16)?, le32(header, 12)?)
}

pub(crate) fn farbfeld(header: &[u8]) -> Option<Extent> {
    Extent::new(be32(header, 8)?, be32(header, 12)?)
}

pub(crate) fn radiance(header: &[u8]) -> Option<Extent> {
    let text = ascii(header);
    let line = text
        .lines()
        .find(|line| line.starts_with("-Y ") || line.starts_with("+Y "))?;
    let mut fields = line.split_ascii_whitespace().skip(1);
    let height = fields.next()?.parse().ok()?;
    let width = fields.nth(1)?.parse().ok()?;
    Extent::new(width, height)
}

pub(crate) fn pnm(header: &[u8]) -> Option<Extent> {
    let text = ascii(header);
    let text = text.as_ref();
    if text.starts_with("P7") {
        let field = |name: &str| {
            text.lines()
                .find_map(|line| line.strip_prefix(name)?.trim().parse().ok())
        };
        return Extent::new(field("WIDTH")?, field("HEIGHT")?);
    }

    let mut fields = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .flat_map(str::split_ascii_whitespace)
        .skip(1);
    Extent::new(fields.next()?.parse().ok()?, fields.next()?.parse().ok()?)
}

const ASCII_HEADER: usize = 1024;

fn ascii(header: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(&header[..header.len().min(ASCII_HEADER)])
}

fn be16(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(
        bytes.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn le16(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(
        bytes.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn le24(bytes: &[u8], at: usize) -> Option<u32> {
    let bytes = bytes.get(at..at + 3)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
