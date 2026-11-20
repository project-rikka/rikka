use rikka_image::{ColorType, Error, Format, Image, read};

use crate::support::gradient;

const IN_HOUSE: [Format; 9] = [
    Format::Png,
    Format::Gif,
    Format::Bmp,
    Format::Ico,
    Format::Tga,
    Format::Qoi,
    Format::Pnm,
    Format::Farbfeld,
    Format::Hdr,
];

struct Random(u64);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    fn below(&mut self, bound: usize) -> usize {
        self.next() as usize % bound
    }
}

fn surface() -> Vec<u8> {
    let mut bytes = b"DDS ".to_vec();
    bytes.extend_from_slice(&124u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&7u32.to_le_bytes());
    bytes.extend_from_slice(&9u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 56]);
    bytes.extend_from_slice(&32u32.to_le_bytes());
    bytes.extend_from_slice(&4u32.to_le_bytes());
    bytes.extend_from_slice(b"DXT5");
    bytes.extend_from_slice(&[0; 40]);
    bytes.extend((0..6 * 16).map(|index| (index * 37 % 251) as u8));
    bytes
}

fn samples() -> Vec<(Format, Vec<u8>)> {
    let source = gradient(9, 7);
    let mut samples: Vec<(Format, Vec<u8>)> = IN_HOUSE
        .into_iter()
        .map(|format| (format, source.encode(format).expect("encodes")))
        .collect();
    samples.push((Format::Dds, surface()));
    samples.push((
        Format::Bmp,
        gradient(9, 7)
            .convert(ColorType::L8)
            .encode(Format::Bmp)
            .expect("encodes"),
    ));
    let plain: Vec<String> = (0..6 * 5 * 3).map(|sample| sample.to_string()).collect();
    samples.push((
        Format::Pnm,
        format!("P3\n6 5\n255\n{}\n", plain.join(" ")).into_bytes(),
    ));
    samples
}

fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut checked = kind.to_vec();
    checked.extend_from_slice(body);
    let mut out = (body.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(&checked);
    out.extend_from_slice(&rikka_deflate::crc32(&checked).to_be_bytes());
    out
}

fn png(width: u32, height: u32, raw: &[u8]) -> Vec<u8> {
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut data = Vec::new();
    rikka_deflate::Deflater::new(rikka_deflate::Level::Best).zlib(raw, &mut data);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend(chunk(b"IHDR", &header));
    out.extend(chunk(b"IDAT", &data));
    out.extend(chunk(b"IEND", &[]));
    out
}

#[test]
fn truncated_files_never_panic_and_never_change_size() {
    for (format, bytes) in samples() {
        let whole = read(&bytes).format(format).decode().expect("decodes");
        for cut in 0..bytes.len() {
            let part = &bytes[..cut];
            if let Ok(image) = read(part).format(format).decode() {
                assert_eq!(image.dimensions(), whole.dimensions(), "{format} cut {cut}");
            }
            let _ = read(part).format(format).probe();
            let _ = read(part).format(format).animation();
            let _ = read(part).decode();
        }
    }
}

#[test]
fn corrupted_files_never_panic_or_outgrow_the_limits() {
    let mut random = Random(77);
    for (format, bytes) in samples() {
        for _ in 0..600 {
            let mut damaged = bytes.clone();
            for _ in 0..1 + random.below(4) {
                let at = random.below(damaged.len());
                damaged[at] = random.next() as u8;
            }
            let request = || {
                read(&damaged)
                    .format(format)
                    .max_alloc(1 << 20)
                    .max_dimensions(1024, 1024)
            };
            if let Ok(image) = request().decode() {
                assert!(image.as_bytes().len() <= 1 << 20, "{format}");
                assert!(image.width() <= 1024 && image.height() <= 1024, "{format}");
            }
            if let Ok(animation) = request().animation() {
                let total: usize = animation
                    .frames()
                    .iter()
                    .map(|frame| frame.image().as_bytes().len())
                    .sum();
                assert!(total <= 1 << 20, "{format}");
            }
            let _ = request().probe();
        }
    }
}

#[test]
fn every_decoder_honours_allocation_and_dimension_limits() {
    for (format, bytes) in samples() {
        let over_budget = read(&bytes).format(format).max_alloc(50).decode();
        assert!(
            matches!(over_budget, Err(Error::LimitsExceeded(_))),
            "{format}: {over_budget:?}"
        );
        let too_large = read(&bytes).format(format).max_dimensions(4, 4).decode();
        assert!(
            matches!(too_large, Err(Error::LimitsExceeded(_))),
            "{format}: {too_large:?}"
        );
        assert!(read(&bytes).format(format).probe().is_ok(), "{format}");
    }
}

#[test]
fn png_checksums_bombs_and_giant_headers_are_refused() {
    let honest = png(2, 2, &[0; 18]);
    assert!(Image::from_bytes(&honest).is_ok());

    let mut wrong_sum = honest.clone();
    let data_at = 8 + 25 + 8;
    wrong_sum[data_at + 3] ^= 0x20;
    assert!(matches!(
        Image::from_bytes(&wrong_sum),
        Err(Error::Decoding(_))
    ));

    let bomb = png(2, 2, &vec![0; 32 << 20]);
    assert!(bomb.len() < 64 << 10);
    assert!(matches!(Image::from_bytes(&bomb), Err(Error::Decoding(_))));

    let giant = png(40_000, 40_000, &[0; 64]);
    assert!(matches!(
        Image::from_bytes(&giant),
        Err(Error::LimitsExceeded(_))
    ));
    assert!(matches!(
        read(&giant).no_alloc_limit().decode(),
        Err(Error::Decoding(_))
    ));
}

#[test]
fn gif_data_beyond_its_frame_is_ignored() {
    let large = Image::filled(
        64,
        64,
        ColorType::Rgb8,
        rikka_types::Color::rgb(0.0, 1.0, 0.0),
    )
    .expect("valid")
    .encode(Format::Gif)
    .expect("encodes");
    let mut small = large.clone();
    small[6..10].copy_from_slice(&[2, 0, 2, 0]);
    let descriptor = small
        .windows(9)
        .position(|window| window == [0x2C, 0, 0, 0, 0, 64, 0, 64, 0])
        .expect("image descriptor");
    small[descriptor + 5..descriptor + 9].copy_from_slice(&[2, 0, 2, 0]);

    let decoded = read(&small).max_alloc(64).decode().expect("decodes");

    assert_eq!(decoded.dimensions(), (2, 2));
    assert_eq!(decoded.as_bytes(), [0, 255, 0, 255].repeat(4));
}
