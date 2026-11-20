use std::io::Cursor;

use rikka_png::{Layout, Picture, Samples};

pub(crate) const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

pub(crate) struct Random(u64);

impl Random {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    pub(crate) fn below(&mut self, bound: usize) -> usize {
        self.next() as usize % bound
    }
}

pub(crate) fn texture(len: usize, seed: u64) -> Vec<u8> {
    let mut random = Random::new(seed);
    let mut value = 0u8;
    (0..len)
        .map(|index| {
            if random.below(5) == 0 {
                value = random.next() as u8;
            }
            value.wrapping_add((index % 7) as u8)
        })
        .collect()
}

pub(crate) fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let mut crc = rikka_deflate::Crc32::new();
    crc.update(kind);
    crc.update(body);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

pub(crate) fn header(width: u32, height: u32, depth: u8, color: u8, interlace: u8) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&width.to_be_bytes());
    body.extend_from_slice(&height.to_be_bytes());
    body.extend_from_slice(&[depth, color, 0, 0, interlace]);
    body
}

pub(crate) fn zlib(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    rikka_deflate::Deflater::new(rikka_deflate::Level::Balanced).zlib(raw, &mut out);
    out
}

pub(crate) fn assemble(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = SIGNATURE.to_vec();
    for (kind, body) in chunks {
        chunk(&mut out, kind, body);
    }
    out
}

pub(crate) fn rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    for row in rgba.chunks_exact(width as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    assemble(&[
        (b"IHDR", header(width, height, 8, 6, 0)),
        (b"IDAT", zlib(&raw)),
        (b"IEND", Vec::new()),
    ])
}

pub(crate) fn chunks(png: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let mut found = Vec::new();
    let mut at = 8;
    while at + 12 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().expect("four")) as usize;
        let kind: [u8; 4] = png[at + 4..at + 8].try_into().expect("four");
        found.push((kind, png[at + 8..at + 8 + length].to_vec()));
        at += 12 + length;
    }
    found
}

pub(crate) fn oracle_decode(png: &[u8]) -> Picture {
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("oracle reads the header");
    let mut buffer = vec![0; reader.output_buffer_size().expect("oracle sizes the frame")];
    let frame = reader.next_frame(&mut buffer).expect("oracle decodes");
    buffer.truncate(frame.buffer_size());
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => panic!("oracle expands palettes"),
    };
    let deep = frame.bit_depth == png::BitDepth::Sixteen;
    let layout = match (channels, deep) {
        (1, false) => Layout::L8,
        (2, false) => Layout::La8,
        (3, false) => Layout::Rgb8,
        (4, false) => Layout::Rgba8,
        (1, true) => Layout::L16,
        (2, true) => Layout::La16,
        (3, true) => Layout::Rgb16,
        _ => Layout::Rgba16,
    };
    let samples = if deep {
        Samples::U16(
            buffer
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect(),
        )
    } else {
        Samples::U8(buffer)
    };
    Picture {
        width: frame.width,
        height: frame.height,
        layout,
        samples,
    }
}

pub(crate) fn to_rgba8(picture: &Picture) -> Vec<u8> {
    let narrow = |value: u16| ((u32::from(value) + 128) / 257) as u8;
    let samples: Vec<u8> = match &picture.samples {
        Samples::U8(samples) => samples.clone(),
        Samples::U16(samples) => samples.iter().copied().map(narrow).collect(),
    };
    samples
        .chunks_exact(picture.layout.channels())
        .flat_map(|pixel| match *pixel {
            [gray] => [gray, gray, gray, 255],
            [gray, alpha] => [gray, gray, gray, alpha],
            [red, green, blue] => [red, green, blue, 255],
            [red, green, blue, alpha] => [red, green, blue, alpha],
            _ => unreachable!("one to four channels"),
        })
        .collect()
}
