use crate::media::Media;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG: &[u8] = b"\xff\xd8";
const START_OF_SCAN: u8 = 0xda;
const END_OF_IMAGE: u8 = 0xd9;
const VP8_START: &[u8] = b"\x9d\x01\x2a";
const VP8L_SIGNATURE: u8 = 0x2f;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picture {
    pub media: Media,
    pub width: u32,
    pub height: u32,
}

impl Picture {
    #[must_use]
    pub fn sniff(data: &[u8]) -> Option<Picture> {
        png(data)
            .or_else(|| gif(data))
            .or_else(|| jpeg(data))
            .or_else(|| webp(data))
            .filter(|picture| picture.width > 0 && picture.height > 0)
    }
}

fn png(data: &[u8]) -> Option<Picture> {
    let header = data.strip_prefix(PNG)?;
    if header.get(4..8)? != b"IHDR" {
        return None;
    }
    Some(Picture {
        media: Media::Png,
        width: big_endian(header.get(8..12)?),
        height: big_endian(header.get(12..16)?),
    })
}

fn gif(data: &[u8]) -> Option<Picture> {
    if !data.starts_with(b"GIF87a") && !data.starts_with(b"GIF89a") {
        return None;
    }
    Some(Picture {
        media: Media::Gif,
        width: little_endian(data.get(6..8)?),
        height: little_endian(data.get(8..10)?),
    })
}

fn jpeg(data: &[u8]) -> Option<Picture> {
    let mut rest = data.strip_prefix(JPEG)?;
    loop {
        let marker = rest.iter().position(|byte| *byte != 0xff)?;
        if marker == 0 {
            return None;
        }
        let kind = rest[marker];
        rest = &rest[marker + 1..];
        match kind {
            START_OF_SCAN | END_OF_IMAGE => return None,
            0x01 | 0xd0..=0xd7 => continue,
            _ => {}
        }

        let length = usize::try_from(big_endian(rest.get(..2)?)).ok()?;
        let segment = rest.get(2..length)?;
        if is_frame_header(kind) {
            return Some(Picture {
                media: Media::Jpeg,
                height: big_endian(segment.get(1..3)?),
                width: big_endian(segment.get(3..5)?),
            });
        }
        rest = &rest[length..];
    }
}

fn is_frame_header(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc)
}

fn webp(data: &[u8]) -> Option<Picture> {
    if data.get(..4)? != b"RIFF" || data.get(8..12)? != b"WEBP" {
        return None;
    }
    let body = data.get(20..)?;
    let (width, height) = match data.get(12..16)? {
        b"VP8 " if body.get(3..6)? == VP8_START => (
            little_endian(body.get(6..8)?) & 0x3fff,
            little_endian(body.get(8..10)?) & 0x3fff,
        ),
        b"VP8L" if *body.first()? == VP8L_SIGNATURE => {
            let packed = little_endian(body.get(1..5)?);
            ((packed & 0x3fff) + 1, ((packed >> 14) & 0x3fff) + 1)
        }
        b"VP8X" => (
            little_endian(body.get(4..7)?) + 1,
            little_endian(body.get(7..10)?) + 1,
        ),
        _ => return None,
    };
    Some(Picture {
        media: Media::WebP,
        width,
        height,
    })
}

fn big_endian(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0, |value, byte| (value << 8) | u32::from(*byte))
}

fn little_endian(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .rev()
        .fold(0, |value, byte| (value << 8) | u32::from(*byte))
}
