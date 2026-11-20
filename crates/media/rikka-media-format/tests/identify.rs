use rikka_image::Format as Picture;
use rikka_media_format::{Detail, Format};

fn file_type(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
    let size = 16 + 4 * compatible.len() as u32;
    let mut bytes = size.to_be_bytes().to_vec();
    bytes.extend_from_slice(b"ftyp");
    bytes.extend_from_slice(major);
    bytes.extend_from_slice(&[0; 4]);
    for brand in compatible {
        bytes.extend_from_slice(*brand);
    }
    bytes
}

#[test]
fn file_type_brands_tell_pictures_from_movies() {
    let cases = [
        (
            "photo.heic",
            file_type(b"heic", &[b"mif1", b"heic"]),
            Detail::Picture(Picture::Heif),
        ),
        (
            "photo.bin",
            file_type(b"mif1", &[b"mif1", b"heix"]),
            Detail::Picture(Picture::Heif),
        ),
        (
            "photo.avif",
            file_type(b"avif", &[b"avif", b"mif1"]),
            Detail::Picture(Picture::Avif),
        ),
        (
            "photo.heif",
            file_type(b"mif1", &[b"mif1", b"avif"]),
            Detail::Picture(Picture::Avif),
        ),
        (
            "clip.avif",
            file_type(b"avis", &[b"avis", b"msf1"]),
            Detail::Picture(Picture::Avif),
        ),
        (
            "clip.mp4",
            file_type(b"isom", &[b"isom", b"iso2"]),
            Detail::Movie,
        ),
        (
            "scan.bin",
            b"\x00\x00\x00\x0CjP  \r\n\x87\n".to_vec(),
            Detail::Picture(Picture::Jpeg2000),
        ),
        (
            "scan.bin",
            vec![0xFF, 0x4F, 0xFF, 0x51],
            Detail::Picture(Picture::Jpeg2000),
        ),
        ("scan.j2k", Vec::new(), Detail::Picture(Picture::Jpeg2000)),
        (
            "photo.bin",
            vec![0xFF, 0x0A, 0xFA, 0x7F],
            Detail::Picture(Picture::JpegXl),
        ),
        (
            "photo.bin",
            b"\x00\x00\x00\x0CJXL \r\n\x87\n".to_vec(),
            Detail::Picture(Picture::JpegXl),
        ),
        ("photo.JXL", Vec::new(), Detail::Picture(Picture::JpegXl)),
        ("photo.HEIC", Vec::new(), Detail::Picture(Picture::Heif)),
    ];
    for (name, header, expected) in cases {
        let format = Format::identify(name, &header).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(format.detail(), expected, "{name}");
    }
}
