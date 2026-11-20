use std::path::Path;
use std::sync::Arc;

use files_convert::Target;
use rikka_image::{Format, Image, read};

use crate::support::{converted, names, succeeded};

const SECRET: &[u8] = b"Secret";
const ORIENTATION_ROTATED: u16 = 6;

fn profile() -> Arc<[u8]> {
    (0..200u8).collect::<Vec<u8>>().into()
}

fn portrait() -> Image {
    let pixels: Vec<u8> = (0..8 * 4)
        .flat_map(|at| [at as u8 * 8, 40, 200, 255])
        .collect();
    Image::from_rgba8(8, 4, pixels)
        .expect("build picture")
        .with_icc_profile(Some(profile()))
}

fn exif(orientation: u16) -> Vec<u8> {
    let maker = b"SecretCam\0";
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42u16.to_le_bytes());
    tiff.extend_from_slice(&8u32.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&0x010Fu16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&(maker.len() as u32).to_le_bytes());
    tiff.extend_from_slice(&38u32.to_le_bytes());
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&0u32.to_le_bytes());
    tiff.extend_from_slice(maker);
    tiff
}

fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut segment = vec![0xFF, marker];
    segment.extend_from_slice(&(payload.len() as u16 + 2).to_be_bytes());
    segment.extend_from_slice(payload);
    segment
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    chunk.extend_from_slice(&crc.finalize().to_be_bytes());
    chunk
}

fn riff_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = kind.to_vec();
    chunk.extend_from_slice(&(data.len() as u32).to_le_bytes());
    chunk.extend_from_slice(data);
    if data.len() % 2 == 1 {
        chunk.push(0);
    }
    chunk
}

fn jpeg_with_secrets() -> Vec<u8> {
    let plain = portrait().encode(Format::Jpeg).expect("encode jpeg");
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&exif(ORIENTATION_ROTATED));
    let mut bytes = plain[..2].to_vec();
    bytes.extend_from_slice(&segment(0xE1, &payload));
    bytes.extend_from_slice(&segment(
        0xE1,
        b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta>Secret place</x:xmpmeta>",
    ));
    bytes.extend_from_slice(&segment(0xFE, b"Secret comment"));
    bytes.extend_from_slice(&plain[2..]);
    bytes.extend_from_slice(b"Secret trailer");
    bytes
}

fn png_with_secrets() -> Vec<u8> {
    let plain = portrait().encode(Format::Png).expect("encode png");
    let idat = plain
        .windows(4)
        .position(|window| window == b"IDAT")
        .expect("pixel data")
        - 4;
    let mut bytes = plain[..idat].to_vec();
    bytes.extend_from_slice(&png_chunk(b"tEXt", b"Comment\0Secret note"));
    bytes.extend_from_slice(&png_chunk(b"eXIf", &exif(ORIENTATION_ROTATED)));
    bytes.extend_from_slice(&png_chunk(b"tIME", &[7, 234, 1, 2, 3, 4, 5]));
    bytes.extend_from_slice(&plain[idat..]);
    bytes
}

fn webp_with_secrets() -> Vec<u8> {
    let plain = portrait().encode(Format::WebP).expect("encode webp");
    let mut bytes = plain.clone();
    bytes.extend_from_slice(&riff_chunk(b"EXIF", &exif(ORIENTATION_ROTATED)));
    bytes.extend_from_slice(&riff_chunk(b"XMP ", b"<x:xmpmeta>Secret place</x:xmpmeta>"));
    bytes[20] |= 0x08 | 0x04;
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes
}

fn stripped(folder: &Path, name: &str, bytes: &[u8]) -> Vec<u8> {
    let source = folder.join(name);
    std::fs::write(&source, bytes).expect("write source");
    succeeded(&[&source], Target::StripMetadata, " (clean)");
    let (stem, extension) = name.rsplit_once('.').expect("file extension");
    std::fs::read(folder.join(format!("{stem} (clean).{extension}"))).expect("read stripped copy")
}

#[test]
fn strip_keeps_pixels_orientation_icc() {
    let folder = tempfile::tempdir().unwrap();

    for (name, original) in [
        ("photo.jpg", jpeg_with_secrets()),
        ("photo.png", png_with_secrets()),
        ("photo.webp", webp_with_secrets()),
    ] {
        let clean = stripped(folder.path(), name, &original);

        assert!(
            !clean.windows(SECRET.len()).any(|window| window == SECRET),
            "{name} keeps a secret"
        );
        let before = read(original.as_slice()).decode().expect("decode original");
        let after = read(clean.as_slice())
            .decode()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(after.dimensions(), (4, 8), "{name} lost its turn");
        assert_eq!(
            after.as_bytes(),
            before.as_bytes(),
            "{name} changed its pixels"
        );
        assert_eq!(
            after.icc_profile(),
            Some(&profile()[..]),
            "{name} lost its colours"
        );
    }
}

#[test]
fn strip_drops_exif_without_orientation() {
    let folder = tempfile::tempdir().unwrap();
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&exif(1));
    let plain = portrait().encode(Format::Jpeg).expect("encode jpeg");
    let mut original = plain[..2].to_vec();
    original.extend_from_slice(&segment(0xE1, &payload));
    original.extend_from_slice(&plain[2..]);

    let clean = stripped(folder.path(), "upright.jpg", &original);

    assert!(!clean.windows(4).any(|window| window == b"Exif"));
    assert_eq!(
        read(clean.as_slice())
            .decode()
            .expect("read stripped copy")
            .dimensions(),
        (8, 4)
    );
}

#[test]
fn damaged_pictures_fail_without_output() {
    let folder = tempfile::tempdir().unwrap();
    let damaged: [(&str, &[u8]); 3] = [
        ("flags.webp", b"RIFF\x0c\0\0\0WEBPVP8X\0\0\0\0"),
        ("segment.jpg", b"\xff\xd8\xff\xe1\xff\xffExif"),
        ("chunk.png", b"\x89PNG\r\n\x1a\n\0\0\xff\xffIHDR"),
    ];
    for (name, bytes) in damaged {
        std::fs::write(folder.path().join(name), bytes).expect("write source");
    }
    let sources: Vec<_> = damaged
        .iter()
        .map(|(name, _)| folder.path().join(name))
        .collect();
    let sources: Vec<&Path> = sources.iter().map(|source| source.as_path()).collect();

    let outcome = converted(&sources, Target::StripMetadata, " (clean)");

    assert_eq!(
        outcome.failures.len(),
        damaged.len(),
        "{:?}",
        outcome.failures
    );
    assert_eq!(
        names(folder.path()),
        ["chunk.png", "flags.webp", "segment.jpg"]
    );
}
