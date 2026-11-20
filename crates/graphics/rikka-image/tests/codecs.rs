use rikka_image::{ColorType, Error, Format, Image, Orientation, Png, Region, WebP, read};

use crate::support::{gradient, opaque_gradient};

#[test]
fn encodable_formats_round_trip() {
    let source = gradient(16, 16);

    for format in Format::ALL {
        if !format.can_encode() {
            continue;
        }
        let bytes = source
            .write(format)
            .speed(10)
            .quality(60)
            .to_bytes()
            .unwrap_or_else(|err| panic!("encoding {format} failed: {err}"));
        assert!(!bytes.is_empty(), "{format} encoded to nothing");
        let decoded = read(&bytes)
            .format(format)
            .decode()
            .unwrap_or_else(|err| panic!("decoding {format} failed: {err}"));
        assert_eq!(
            decoded.dimensions(),
            source.dimensions(),
            "{format} changed the size"
        );
    }
}

#[test]
fn lossless_formats_preserve_pixels() {
    let source = opaque_gradient(16, 16);

    for format in [
        Format::Png,
        Format::Bmp,
        Format::Tiff,
        Format::Qoi,
        Format::Tga,
        Format::Pnm,
        Format::Farbfeld,
    ] {
        let bytes = source.encode(format).expect("encodes");
        let decoded = read(&bytes).format(format).decode().expect("decodes");
        assert_eq!(
            decoded.convert(ColorType::Rgb8).as_bytes(),
            source.convert(ColorType::Rgb8).as_bytes(),
            "{format} altered the pixels"
        );
    }
}

#[test]
fn png_keeps_depth_and_alpha_jpeg_drops_them() {
    let deep = gradient(8, 8).convert(ColorType::Rgba16);
    let png = Image::from_bytes(&deep.encode(Format::Png).expect("encodes")).expect("decodes");
    assert_eq!(png.color(), ColorType::Rgba16);
    assert_eq!(png.as_bytes(), deep.as_bytes());

    let gray = gradient(8, 8).convert(ColorType::La8);
    let png = Image::from_bytes(&gray.encode(Format::Png).expect("encodes")).expect("decodes");
    assert_eq!(png.color(), ColorType::La8);

    let jpeg = Image::from_bytes(&deep.encode(Format::Jpeg).expect("encodes")).expect("decodes");
    assert_eq!(jpeg.color(), ColorType::Rgb8);
    assert!(!jpeg.has_alpha());
}

#[test]
fn formats_are_sniffed_from_contents() {
    let source = gradient(8, 8);

    for format in [Format::Png, Format::Jpeg, Format::WebP, Format::Gif] {
        let bytes = source.encode(format).expect("encodes");
        assert_eq!(Format::detect(&bytes), Some(format));
        assert!(Image::from_bytes(&bytes).is_ok(), "{format} did not sniff");
    }
}

#[test]
fn probe_reports_info_without_decoding() {
    let bytes = gradient(37, 11).encode(Format::Png).expect("encodes");

    let info = read(&bytes).probe().expect("probes");

    assert_eq!(info.format, Format::Png);
    assert_eq!(info.dimensions(), (37, 11));
    assert_eq!(info.color, ColorType::Rgba8);
    assert!(info.has_alpha());
}

#[test]
fn rejects_unknown_formats_and_extensions() {
    let unknown = Image::from_bytes(b"this is not an image").expect_err("not an image");
    assert!(matches!(unknown, Error::UnknownFormat), "{unknown:?}");

    let unwritable = gradient(4, 4)
        .encode(Format::Dds)
        .expect_err("DDS has no encoder");
    assert!(
        matches!(unwritable, Error::Unsupported(_)),
        "{unwritable:?}"
    );

    let unnamed = gradient(4, 4)
        .save("/nonexistent/photo.txt")
        .expect_err("no format for .txt");
    assert!(matches!(unnamed, Error::UnknownExtension(_)), "{unnamed:?}");
}

#[test]
fn enforces_decoding_limits() {
    let bytes = gradient(64, 64).encode(Format::Png).expect("encodes");

    let over_budget = read(&bytes)
        .max_alloc(1024)
        .decode()
        .expect_err("64x64 RGBA8 needs 16 KiB");
    assert!(
        matches!(over_budget, Error::LimitsExceeded(_)),
        "{over_budget:?}"
    );

    let too_large = read(&bytes)
        .max_dimensions(16, 16)
        .decode()
        .expect_err("64 pixels exceed 16 pixel limit");
    assert!(
        matches!(too_large, Error::LimitsExceeded(_)),
        "{too_large:?}"
    );

    assert!(read(&bytes).no_alloc_limit().decode().is_ok());
}

#[test]
fn webp_lossy_by_default_lossless_on_request() {
    let source = gradient(64, 64);

    let exact = source.write(WebP).lossless().to_bytes().expect("encodes");
    let decoded = read(&exact).format(Format::WebP).decode().expect("decodes");
    assert_eq!(
        decoded.convert(ColorType::Rgba8).as_bytes(),
        source.as_bytes()
    );

    let lossy = source.write(WebP).quality(10).to_bytes().expect("encodes");
    let rough = read(&lossy).format(Format::WebP).decode().expect("decodes");
    assert_eq!(rough.dimensions(), source.dimensions());
    assert_ne!(
        rough.convert(ColorType::Rgba8).as_bytes(),
        source.as_bytes()
    );
}

#[test]
fn icc_profile_round_trips() {
    let profile: std::sync::Arc<[u8]> = (0..200u8).collect::<Vec<u8>>().into();
    let source = opaque_gradient(8, 8).with_icc_profile(Some(profile.clone()));

    for format in [Format::Png, Format::Jpeg, Format::WebP] {
        let bytes = source.encode(format).expect("encodes");
        let decoded = Image::from_bytes(&bytes).expect("decodes");
        assert_eq!(decoded.icc_profile(), Some(&profile[..]), "{format}");
        let animation = read(&bytes).animation().expect("decodes");
        assert_eq!(
            animation.still().icc_profile(),
            Some(&profile[..]),
            "{format}"
        );
    }
    let translucent = gradient(8, 8).with_icc_profile(Some(profile.clone()));
    let opaque_with_alpha = opaque_gradient(8, 8)
        .convert(ColorType::Rgba8)
        .with_icc_profile(Some(profile.clone()));
    for picture in [translucent, opaque_with_alpha] {
        for bytes in [
            picture.encode(WebP).expect("encodes"),
            picture.write(WebP).lossless().to_bytes().expect("encodes"),
        ] {
            let decoded = Image::from_bytes(&bytes).expect("decodes");
            assert_eq!(decoded.icc_profile(), Some(&profile[..]));
        }
    }
    assert_eq!(
        source
            .crop(Region::new(2, 2, 4, 4))
            .expect("crops")
            .icc_profile(),
        Some(&profile[..])
    );
    let untagged = opaque_gradient(4, 4).encode(Format::Png).expect("encodes");
    assert_eq!(
        Image::from_bytes(&untagged).expect("decodes").icc_profile(),
        None
    );
}

#[test]
fn every_layout_a_format_stores_comes_back_exactly() {
    use ColorType::{L8, L16, La8, La16, Rgb8, Rgb16, Rgba8, Rgba16};

    let cases: [(Format, &[ColorType]); 7] = [
        (
            Format::Png,
            &[L8, La8, Rgb8, Rgba8, L16, La16, Rgb16, Rgba16],
        ),
        (
            Format::Pnm,
            &[L8, La8, Rgb8, Rgba8, L16, La16, Rgb16, Rgba16],
        ),
        (Format::Bmp, &[L8, Rgb8, Rgba8]),
        (Format::Tga, &[L8, La8, Rgb8, Rgba8]),
        (Format::Qoi, &[Rgb8, Rgba8]),
        (Format::Ico, &[Rgba8]),
        (Format::Farbfeld, &[Rgba16]),
    ];
    for (format, layouts) in cases {
        for &layout in layouts {
            for (width, height) in [(1, 1), (5, 3), (130, 7)] {
                let source = gradient(width, height).convert(layout);
                let bytes = source.encode(format).expect("encodes");
                let decoded = read(&bytes)
                    .format(format)
                    .decode()
                    .unwrap_or_else(|error| {
                        panic!("{format} {layout:?} {width}x{height}: {error}")
                    });
                assert_eq!(
                    decoded.convert(layout),
                    source,
                    "{format} {layout:?} {width}x{height}"
                );
            }
        }
    }

    let bright = Image::from_rgba32f(2, 1, vec![0.25, 1.0, 4.0, 1.0, 0.0, 1000.0, 0.001, 1.0])
        .expect("valid");
    let bytes = bright.encode(Format::Hdr).expect("encodes");
    let decoded = read(&bytes).decode().expect("decodes");
    assert_eq!(decoded.color(), ColorType::Rgb32F);
    let samples = decoded.rgba32f();
    let (texels, _) = samples.as_chunks::<4>();
    for (texel, expected) in texels.iter().zip([[0.25, 1.0, 4.0], [0.0, 1000.0, 0.0]]) {
        let step = expected.iter().copied().fold(0.0f32, f32::max) / 128.0;
        for (written, wanted) in texel[..3].iter().zip(expected) {
            assert!((written - wanted).abs() <= step, "{texel:?}");
        }
        assert_eq!(texel[3], 1.0);
    }
    let wide = Image::filled(
        40,
        3,
        ColorType::Rgb32F,
        rikka_types::Color::rgb(0.5, 2.0, 8.0),
    )
    .expect("valid");
    let decoded = read(wide.encode(Format::Hdr).expect("encodes"))
        .decode()
        .expect("decodes");
    assert_eq!(decoded, wide);
}

#[test]
fn png_text_travels_with_the_file_and_is_probed_without_decoding() {
    let fields = vec![
        (
            "Thumb::URI".to_owned(),
            "file:///home/person/a%20b.png".to_owned(),
        ),
        ("Thumb::MTime".to_owned(), "1700000000".to_owned()),
        ("Title".to_owned(), "\u{5199}\u{771f}".to_owned()),
    ];
    let bytes = gradient(5, 4)
        .write(Png)
        .text(&fields)
        .to_bytes()
        .expect("encodes");

    let info = read(&bytes).probe().expect("probes");

    assert_eq!(info.text, fields);
    assert_eq!(info.dimensions(), (5, 4));
    let folder = tempfile::tempdir().expect("create temp dir");
    let path = folder.path().join("thumbnail.png");
    std::fs::write(&path, &bytes).expect("writes");
    assert_eq!(read(path.as_path()).probe().expect("probes").text, fields);
    assert!(
        read(gradient(2, 2).encode(Format::Jpeg).expect("encodes"))
            .probe()
            .expect("probes")
            .text
            .is_empty()
    );
}

#[test]
fn png_orientation_data_turns_the_picture() {
    let source = gradient(3, 2);
    let plain = source.encode(Format::Png).expect("encodes");
    let mut exif = b"MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0".to_vec();
    let mut chunk = (exif.len() as u32).to_be_bytes().to_vec();
    let mut body = b"eXIf".to_vec();
    body.append(&mut exif);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&rikka_deflate::crc32(&body).to_be_bytes());
    let mut turned = plain[..33].to_vec();
    turned.extend_from_slice(&chunk);
    turned.extend_from_slice(&plain[33..]);

    let info = read(&turned).probe().expect("probes");
    let decoded = read(&turned).decode().expect("decodes");

    assert_eq!(info.orientation, Orientation::Rotate90);
    assert_eq!(info.dimensions(), (2, 3));
    assert_eq!(decoded.dimensions(), (2, 3));
    assert_eq!(decoded.pixel(1, 0), source.pixel(0, 0));
    assert_eq!(decoded.pixel(0, 2), source.pixel(2, 1));
}
