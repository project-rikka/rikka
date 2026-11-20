use rikka_image::{ColorType, Format, Image, read};

fn rgba(image: &Image) -> Vec<[u8; 4]> {
    image.rgba8().as_chunks::<4>().0.to_vec()
}

fn decode(bytes: &[u8], format: Format) -> Image {
    read(bytes)
        .format(format)
        .decode()
        .unwrap_or_else(|error| panic!("{format}: {error}"))
}

pub(crate) struct Bitmap<'b> {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) bits: u16,
    pub(crate) compression: u32,
    pub(crate) masks: &'b [u32],
    pub(crate) palette: &'b [[u8; 3]],
    pub(crate) data: &'b [u8],
}

impl Bitmap<'_> {
    pub(crate) fn header(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&self.bits.to_le_bytes());
        out.extend_from_slice(&self.compression.to_le_bytes());
        out.extend_from_slice(&[0; 12]);
        out.extend_from_slice(&(self.palette.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        for mask in self.masks {
            out.extend_from_slice(&mask.to_le_bytes());
        }
        for &[red, green, blue] in self.palette {
            out.extend_from_slice(&[blue, green, red, 0]);
        }
        out
    }

    pub(crate) fn file(&self) -> Vec<u8> {
        let header = self.header();
        let mut out = b"BM".to_vec();
        out.extend_from_slice(&((14 + header.len() + self.data.len()) as u32).to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&((14 + header.len()) as u32).to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(self.data);
        out
    }
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const PALETTE: [[u8; 3]; 4] = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];

#[test]
fn bitmaps_of_every_stored_form_decode_to_the_same_picture() {
    let expected = vec![RED, GREEN, BLUE, BLUE, WHITE, RED];
    let plain = Bitmap {
        width: 3,
        height: 2,
        bits: 24,
        compression: 0,
        masks: &[],
        palette: &[],
        data: &[
            255, 0, 0, 255, 255, 255, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0, 0,
        ],
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("24-bit bottom-up", plain.file()),
        (
            "24-bit top-down",
            Bitmap {
                height: -2,
                data: &[
                    0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0, 255,
                    0, 0, 0,
                ],
                ..plain
            }
            .file(),
        ),
        (
            "8-bit indexed",
            Bitmap {
                bits: 8,
                palette: &PALETTE,
                data: &[2, 3, 0, 0, 0, 1, 2, 0],
                ..plain
            }
            .file(),
        ),
        (
            "4-bit indexed",
            Bitmap {
                bits: 4,
                palette: &PALETTE,
                data: &[0x23, 0x00, 0, 0, 0x01, 0x20, 0, 0],
                ..plain
            }
            .file(),
        ),
        (
            "8-bit run-length",
            Bitmap {
                bits: 8,
                compression: 1,
                palette: &PALETTE,
                data: &[1, 2, 0, 3, 3, 0, 9, 0, 0, 0, 1, 0, 1, 1, 1, 2, 0, 1],
                ..plain
            }
            .file(),
        ),
        (
            "16-bit 565 fields",
            Bitmap {
                bits: 16,
                compression: 3,
                masks: &[0xF800, 0x07E0, 0x001F],
                data: &[
                    0x1F, 0x00, 0xFF, 0xFF, 0x00, 0xF8, 0, 0, 0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00,
                    0, 0,
                ],
                ..plain
            }
            .file(),
        ),
        (
            "32-bit without alpha",
            Bitmap {
                bits: 32,
                data: &[
                    255, 0, 0, 9, 255, 255, 255, 9, 0, 0, 255, 9, 0, 0, 255, 9, 0, 255, 0, 9, 255,
                    0, 0, 9,
                ],
                ..plain
            }
            .file(),
        ),
    ];

    for (name, bytes) in cases {
        assert_eq!(Format::detect(&bytes), Some(Format::Bmp), "{name}");
        let image = read(&bytes)
            .decode()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(image.dimensions(), (3, 2), "{name}");
        assert_eq!(image.color(), ColorType::Rgb8, "{name}");
        assert_eq!(rgba(&image), expected, "{name}");
    }

    let keyed = Bitmap {
        bits: 32,
        compression: 3,
        masks: &[0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000],
        width: 1,
        height: 1,
        data: &[3, 2, 1, 128],
        palette: &[],
    };
    let mut bytes = keyed.file();
    bytes[14] = 56;
    let image = read(&bytes).decode().expect("alpha fields");
    assert_eq!(image.color(), ColorType::Rgba8);
    assert_eq!(rgba(&image), [[1, 2, 3, 128]]);
}

#[test]
fn icons_pick_the_largest_entry_and_apply_the_mask() {
    let small = Bitmap {
        width: 1,
        height: 2,
        bits: 32,
        compression: 0,
        masks: &[],
        palette: &[],
        data: &[1, 2, 3, 4, 0, 0, 0, 0],
    }
    .header();
    let masked = Bitmap {
        width: 2,
        height: 4,
        bits: 24,
        compression: 0,
        masks: &[],
        palette: &[],
        data: &[
            0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0, 0x40, 0, 0, 0, 0x80, 0, 0,
            0,
        ],
    }
    .header();
    let mut masked_file = masked.clone();
    masked_file.extend_from_slice(&[
        0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 255, 255, 255, 0, 0, 0x40, 0, 0, 0, 0x80, 0, 0, 0,
    ]);
    let mut small_file = small.clone();
    small_file.extend_from_slice(&[1, 2, 3, 4, 0, 0, 0, 0]);

    let mut icon = vec![0, 0, 1, 0, 2, 0];
    let first_at = 6 + 16 * 2;
    for (side, bits, content, at) in [
        (1u8, 32u16, &small_file, first_at),
        (2, 24, &masked_file, first_at + small_file.len()),
    ] {
        icon.extend_from_slice(&[side, side, 0, 0, 1, 0]);
        icon.extend_from_slice(&bits.to_le_bytes());
        icon.extend_from_slice(&(content.len() as u32).to_le_bytes());
        icon.extend_from_slice(&(at as u32).to_le_bytes());
    }
    icon.extend_from_slice(&small_file);
    icon.extend_from_slice(&masked_file);

    assert_eq!(Format::detect(&icon), Some(Format::Ico));
    let info = read(&icon).probe().expect("probes");
    assert_eq!((info.dimensions(), info.color), ((2, 2), ColorType::Rgba8));
    let image = read(&icon).decode().expect("decodes");
    assert_eq!(
        rgba(&image),
        [[0, 0, 255, 0], WHITE, RED, [0, 255, 0, 0]],
        "bottom-up colours with the mask's set bits cleared"
    );

    let written = Image::from_rgba8(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8])
        .expect("valid")
        .encode(Format::Ico)
        .expect("encodes");
    assert_eq!(
        rgba(&decode(&written, Format::Ico)),
        [[1, 2, 3, 4], [5, 6, 7, 8]]
    );
    let oversized = Image::new(257, 1, ColorType::Rgba8).expect("valid");
    assert!(oversized.encode(Format::Ico).is_err());
}

#[test]
fn targa_origins_run_lengths_and_colour_maps_decode() {
    let header = |kind: u8, map: [u8; 5], width: u16, height: u16, bits: u8, descriptor: u8| {
        let mut out = vec![0, u8::from(map[2] != 0), kind];
        out.extend_from_slice(&map);
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&[bits, descriptor]);
        out
    };
    let mut bottom_up = header(2, [0; 5], 2, 2, 24, 0);
    bottom_up.extend_from_slice(&[255, 0, 0, 0, 0, 0, 0, 0, 255, 0, 255, 0]);
    assert_eq!(
        rgba(&decode(&bottom_up, Format::Tga)),
        [RED, GREEN, BLUE, BLACK]
    );

    let mut runs = header(10, [0; 5], 3, 2, 32, 0x28);
    runs.extend_from_slice(&[0x83, 0, 0, 255, 128, 0x01, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        rgba(&decode(&runs, Format::Tga)),
        [
            [255, 0, 0, 128],
            [255, 0, 0, 128],
            [255, 0, 0, 128],
            [255, 0, 0, 128],
            [3, 2, 1, 4],
            [7, 6, 5, 8]
        ]
    );

    let mut mapped = header(1, [0, 0, 2, 0, 24], 2, 1, 8, 0x20);
    mapped.extend_from_slice(&[0, 0, 255, 255, 0, 0, 1, 0]);
    assert_eq!(rgba(&decode(&mapped, Format::Tga)), [BLUE, RED]);

    let mut gray = header(3, [0; 5], 2, 1, 8, 0x20);
    gray.extend_from_slice(&[7, 200]);
    let image = decode(&gray, Format::Tga);
    assert_eq!(image.color(), ColorType::L8);
    assert_eq!(image.as_bytes(), [7, 200]);
}

#[test]
fn portable_maps_of_every_kind_decode() {
    let cases: Vec<(&[u8], ColorType, Vec<u8>)> = vec![
        (
            b"P1\n# a comment\n3 2\n1 0 1\n0 1 0\n",
            ColorType::L8,
            vec![0, 255, 0, 255, 0, 255],
        ),
        (b"P2 2 1 3\n0 3", ColorType::L8, vec![0, 255]),
        (
            b"P3 1 2 255 1 2 3\n4 5 6",
            ColorType::Rgb8,
            vec![1, 2, 3, 4, 5, 6],
        ),
        (
            b"P4\n10 1\n\xA0\x40",
            ColorType::L8,
            vec![0, 255, 0, 255, 255, 255, 255, 255, 255, 0],
        ),
        (b"P5 2 1 255\n\x07\xC8", ColorType::L8, vec![7, 200]),
        (
            b"P6 1 1 65535\n\x01\x02\x03\x04\x05\x06",
            ColorType::Rgb16,
            [0x0102u16, 0x0304, 0x0506]
                .iter()
                .flat_map(|sample| sample.to_ne_bytes())
                .collect(),
        ),
        (
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n\x01\x02\x03\x04",
            ColorType::Rgba8,
            vec![1, 2, 3, 4],
        ),
    ];

    for (bytes, color, expected) in cases {
        let name = String::from_utf8_lossy(&bytes[..2]).into_owned();
        assert_eq!(Format::detect(bytes), Some(Format::Pnm), "{name}");
        let image = read(bytes)
            .decode()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(image.color(), color, "{name}");
        assert_eq!(image.as_bytes(), expected, "{name}");
    }
}

#[test]
fn quite_ok_operations_decode() {
    let mut bytes = b"qoif".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, 7, 0, 0, 0, 1, 4, 0]);
    bytes.extend_from_slice(&[0xFE, 10, 20, 30]);
    bytes.extend_from_slice(&[0xC1]);
    bytes.extend_from_slice(&[0x40 | 3 << 4 | 2 << 2 | 1]);
    bytes.extend_from_slice(&[0x80 | 40, 10 << 4 | 5]);
    bytes.extend_from_slice(&[0xFF, 1, 2, 3, 4]);
    let seen = (10 * 3 + 20 * 5 + 30 * 7 + 255 * 11) % 64;
    bytes.push(seen as u8);
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);

    assert_eq!(
        rgba(&decode(&bytes, Format::Qoi)),
        [
            [10, 20, 30, 255],
            [10, 20, 30, 255],
            [10, 20, 30, 255],
            [11, 20, 29, 255],
            [21, 28, 34, 255],
            [1, 2, 3, 4],
            [10, 20, 30, 255],
        ]
    );
}

#[test]
fn radiance_scanlines_flat_and_packed_decode() {
    let mut flat = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n".to_vec();
    flat.extend_from_slice(&[128, 64, 0, 129, 128, 128, 128, 128]);
    let image = decode(&flat, Format::Hdr);
    assert_eq!(image.color(), ColorType::Rgb32F);
    let samples: Vec<f32> = image
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_ne_bytes(*bytes))
        .collect();
    assert_eq!(samples, [1.0, 0.5, 0.0, 0.5, 0.5, 0.5]);

    let mut packed = b"#?RGBE\nEXPOSURE=1\n\n+Y 2 +X 8\n".to_vec();
    for value in [129u8, 130] {
        packed.extend_from_slice(&[2, 2, 0, 8]);
        packed.extend_from_slice(&[128 + 8, 128]);
        packed.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0, 0]);
        packed.extend_from_slice(&[128 + 4, 0, 128 + 4, 0]);
        packed.extend_from_slice(&[128 + 8, value]);
    }
    let image = decode(&packed, Format::Hdr);
    assert_eq!(image.dimensions(), (8, 2));
    assert_eq!(image.pixel(0, 0).map(|color| color.r), Some(2.0));
    assert_eq!(image.pixel(7, 1).map(|color| color.r), Some(1.0));
}

#[test]
fn block_compressed_surfaces_decode() {
    let header = |four_cc: &[u8; 4], width: u32, height: u32| {
        let mut out = b"DDS ".to_vec();
        out.extend_from_slice(&124u32.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&[0; 56]);
        out.extend_from_slice(&32u32.to_le_bytes());
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(four_cc);
        out.extend_from_slice(&[0; 40]);
        out
    };
    let mut opaque = header(b"DXT1", 4, 2);
    opaque.extend_from_slice(&[0x00, 0xF8, 0x1F, 0x00, 0b1110_0100, 0b0001_1011, 0, 0]);
    let image = decode(&opaque, Format::Dds);
    assert_eq!(image.color(), ColorType::Rgb8);
    assert_eq!(
        rgba(&image),
        [
            RED,
            BLUE,
            [170, 0, 85, 255],
            [85, 0, 170, 255],
            [85, 0, 170, 255],
            [170, 0, 85, 255],
            BLUE,
            RED
        ]
    );

    let mut smooth = header(b"DXT5", 2, 1);
    smooth.extend_from_slice(&[255, 0, 0b0000_1000, 0, 0, 0, 0, 0]);
    smooth.extend_from_slice(&[0xE0, 0x07, 0xE0, 0x07, 0, 0, 0, 0]);
    assert_eq!(
        rgba(&decode(&smooth, Format::Dds)),
        [[0, 255, 0, 255], [0, 255, 0, 0]]
    );
}
