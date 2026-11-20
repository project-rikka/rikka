use std::time::Duration;

use rikka_image::{Avif, ColorType, Error, Format, Image, Loops, Orientation, read};
use rikka_types::Color;

use crate::support::gradient;

const GRID_AVIF: &[u8] = include_bytes!("fixtures/grid.avif");
const ORIENTED_AVIF: &[u8] = include_bytes!("fixtures/oriented.avif");
const ANIMATED_AVIF: &[u8] = include_bytes!("fixtures/animated.avif");
const PHOTO_HEIC: &[u8] = include_bytes!("fixtures/photo.heic");
const TILES_HEIC: &[u8] = include_bytes!("fixtures/tiles.heic");
const PICTURE_JP2: &[u8] = include_bytes!("fixtures/picture.jp2");
const DEEP_J2K: &[u8] = include_bytes!("fixtures/deep.j2k");
const ORIENTED_TIFF: &[u8] = include_bytes!("fixtures/oriented.tif");
const HALF_EXR: &[u8] = include_bytes!("fixtures/half.exr");
const PROFILE: &[u8] = include_bytes!("fixtures/profile.icc");
const LOSSLESS_JXL: &[u8] = include_bytes!("fixtures/lossless.jxl");
const LOSSY_JXL: &[u8] = include_bytes!("fixtures/lossy.jxl");
const DEEP_JXL: &[u8] = include_bytes!("fixtures/deep.jxl");
const FLOAT_JXL: &[u8] = include_bytes!("fixtures/float.jxl");
const ORIENTED_JXL: &[u8] = include_bytes!("fixtures/oriented.jxl");
const ANIMATED_JXL: &[u8] = include_bytes!("fixtures/animated.jxl");
const GAMMA_PROFILE: &[u8] = include_bytes!("fixtures/gamma.icc");
const HUGE_JXL: &[u8] = &[0xFF, 0x0A, 0xFE, 0xFF, 0xFF, 0xFF, 0x13];

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const HALF_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 128.0 / 255.0];
const LOSSY: f32 = 0.08;
const LOSSY_XYB: f32 = 0.15;

type Corners = [[f32; 4]; 4];

fn near(image: &Image, x: u32, y: u32, expected: [f32; 4], tolerance: f32) {
    let Color { r, g, b, a } = image.pixel(x, y).expect("pixel in bounds");
    let found = [r, g, b, a];
    let close = found
        .iter()
        .zip(expected)
        .all(|(found, expected)| (found - expected).abs() <= tolerance);
    assert!(close, "pixel {x},{y} is {found:?}, expected {expected:?}");
}

fn corners(image: &Image, expected: Corners, tolerance: f32) {
    let (right, bottom) = (image.width() - 2, image.height() - 2);
    near(image, 1, 1, expected[0], tolerance);
    near(image, right, 1, expected[1], tolerance);
    near(image, 1, bottom, expected[2], tolerance);
    near(image, right, bottom, expected[3], tolerance);
}

fn deep_gradient(width: u32, height: u32) -> Image {
    let mut samples = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            samples.extend_from_slice(&[
                (x * 65535 / (width - 1)) as u16,
                (y * 65535 / (height - 1)) as u16,
                40000,
                (65535 - x * 30000 / width) as u16,
            ]);
        }
    }
    Image::from_rgba16(width, height, samples).expect("buffer matches dimensions")
}

#[test]
fn ffmpeg_formats_round_trip_at_their_depth() {
    let deep = deep_gradient(48, 40);
    let float = deep.convert(ColorType::Rgba32F);
    let cases: [(Format, Image, ColorType, f32); 7] = [
        (Format::Avif, gradient(48, 40), ColorType::Rgba8, LOSSY),
        (Format::Avif, deep.clone(), ColorType::Rgba16, LOSSY),
        (
            Format::Avif,
            gradient(48, 40).convert(ColorType::La8),
            ColorType::La8,
            LOSSY,
        ),
        (Format::Tiff, deep.clone(), ColorType::Rgba16, 0.0),
        (
            Format::Tiff,
            gradient(48, 40).convert(ColorType::La8),
            ColorType::La8,
            0.0,
        ),
        (Format::OpenExr, float, ColorType::Rgba32F, 0.0),
        (
            Format::JpegXl,
            gradient(48, 40),
            ColorType::Rgba8,
            LOSSY_XYB,
        ),
    ];
    for (format, source, color, tolerance) in cases {
        let bytes = source
            .encode(format)
            .unwrap_or_else(|error| panic!("{format}: {error}"));
        assert_eq!(
            Format::detect(&bytes),
            Some(format),
            "{format} is recognised"
        );
        let info = read(&bytes).probe().expect("probes");
        assert_eq!(
            (info.width, info.height, info.color),
            (48, 40, color),
            "{format}"
        );
        let decoded = read(&bytes)
            .decode()
            .unwrap_or_else(|error| panic!("{format}: {error}"));
        assert_eq!(decoded.color(), color, "{format} keeps its layout");
        for (x, y) in [(0, 0), (47, 0), (20, 20), (0, 39), (47, 39)] {
            let Color { r, g, b, a } = source.pixel(x, y).expect("pixel in bounds");
            near(&decoded, x, y, [r, g, b, a], tolerance.max(1e-6));
        }
    }
}

#[test]
fn avif_writes_ten_bits_for_deep_pictures() {
    let bytes = deep_gradient(32, 32)
        .write(Avif)
        .quality(100)
        .to_bytes()
        .expect("encodes");
    let decoded = read(&bytes).decode().expect("decodes");
    let step = |x: u32| decoded.pixel(x, 0).expect("pixel in bounds").r;
    let distinct = (0..32).map(step).collect::<Vec<_>>();
    assert!(
        distinct.windows(2).all(|pair| pair[1] > pair[0]),
        "{distinct:?}"
    );
}

#[test]
fn avif_configuration_matches_ffmpeg_muxer() {
    let cases: [(Image, &[[u8; 4]]); 4] = [
        (gradient(16, 16), &[[0x81, 0x20, 0, 0], [0x81, 0, 0x1c, 0]]),
        (
            gradient(16, 16).convert(ColorType::La8),
            &[[0x81, 0, 0x1c, 0], [0x81, 0, 0x1c, 0]],
        ),
        (
            deep_gradient(16, 16),
            &[[0x81, 0x20, 0x40, 0], [0x81, 0, 0x5c, 0]],
        ),
        (
            gradient(16, 16).convert(ColorType::L8),
            &[[0x81, 0, 0x1c, 0]],
        ),
    ];
    for (picture, expected) in cases {
        let bytes = picture.encode(Format::Avif).expect("encodes");
        let configs: Vec<[u8; 4]> = bytes
            .windows(8)
            .filter(|window| &window[..4] == b"av1C")
            .map(|window| [window[4], window[5], window[6], window[7]])
            .collect();
        assert_eq!(configs, expected, "{:?}", picture.color());
    }
}

#[test]
fn avif_and_tiff_embed_the_picture_profile() {
    let pictures = [
        gradient(24, 16),
        gradient(24, 16).convert(ColorType::La8),
        deep_gradient(24, 16),
    ];
    for format in [Format::Avif, Format::Tiff] {
        for picture in &pictures {
            let profiled = picture.clone().with_icc_profile(Some(PROFILE.into()));
            let bytes = profiled
                .encode(format)
                .unwrap_or_else(|error| panic!("{format}: {error}"));
            let decoded = read(&bytes)
                .decode()
                .unwrap_or_else(|error| panic!("{format}: {error}"));
            assert_eq!(decoded.icc_profile(), Some(PROFILE), "{format}");
            assert_eq!(decoded.color(), picture.color(), "{format}");
        }
    }
}

#[test]
fn heif_items_keep_orientation_alpha_profile_and_aperture() {
    let oriented = read(ORIENTED_AVIF).decode().expect("oriented AVIF decodes");
    assert_eq!(oriented.dimensions(), (48, 64));
    assert_eq!(oriented.color(), ColorType::Rgba16);
    assert_eq!(oriented.icc_profile(), Some(PROFILE));
    corners(&oriented, [RED, BLUE, GREEN, HALF_WHITE], LOSSY);
    let info = read(ORIENTED_AVIF).probe().expect("oriented AVIF probes");
    assert_eq!((info.width, info.height), (48, 64));
    assert_eq!(info.orientation, Orientation::Rotate90FlipHorizontal);

    let photo = read(PHOTO_HEIC).decode().expect("HEIC decodes");
    assert_eq!(photo.dimensions(), (64, 48));
    assert_eq!(photo.color(), ColorType::Rgba8);
    corners(&photo, [RED, GREEN, BLUE, HALF_WHITE], LOSSY);
}

#[test]
fn heif_grids_assemble_their_tiles() {
    let grid = read(GRID_AVIF).decode().expect("AVIF grid decodes");
    assert_eq!(
        (grid.dimensions(), grid.color()),
        ((128, 128), ColorType::Rgba8)
    );
    corners(&grid, [RED, GREEN, BLUE, HALF_WHITE], LOSSY);
    near(&grid, 63, 63, RED, LOSSY);
    near(&grid, 64, 64, HALF_WHITE, LOSSY);

    let tiles = read(TILES_HEIC).decode().expect("HEIC grid decodes");
    assert_eq!(
        (tiles.dimensions(), tiles.color()),
        ((128, 128), ColorType::Rgb16)
    );
    for (x, y) in [(0, 0), (63, 64), (64, 63), (127, 127)] {
        let expected = [x as f32 / 127.0, y as f32 / 127.0, 0.5, 1.0];
        near(&tiles, x, y, expected, 0.03);
    }
}

#[test]
fn animated_avif_keeps_frame_timing_loops_and_alpha() {
    let animation = read(ANIMATED_AVIF).animation().expect("animation decodes");
    let delays: Vec<Duration> = animation
        .frames()
        .iter()
        .map(|frame| frame.delay())
        .collect();
    assert_eq!(
        delays,
        [100, 200, 300].map(Duration::from_millis),
        "frame timing"
    );
    assert_eq!(animation.loops(), Loops::Finite(3));
    let frames = animation.frames();
    near(frames[0].image(), 10, 10, RED, LOSSY);
    near(
        frames[1].image(),
        10,
        10,
        [0.0, 1.0, 0.0, 128.0 / 255.0],
        LOSSY,
    );
    near(frames[2].image(), 10, 10, BLUE, LOSSY);

    let still = read(ANIMATED_AVIF).decode().expect("first picture decodes");
    near(&still, 10, 10, RED, LOSSY);
}

#[test]
fn jpeg2000_files_and_codestreams_decode() {
    let picture = read(PICTURE_JP2).decode().expect("JP2 decodes");
    assert_eq!(
        (picture.dimensions(), picture.color()),
        ((64, 48), ColorType::Rgba8)
    );
    corners(&picture, [RED, GREEN, BLUE, HALF_WHITE], 0.0);

    let deep = read(DEEP_J2K).decode().expect("J2K decodes");
    assert_eq!(
        (deep.dimensions(), deep.color()),
        ((128, 128), ColorType::Rgb16)
    );
    near(&deep, 127, 0, [1.0, 0.0, 0.5, 1.0], 0.001);
}

#[test]
fn tiff_keeps_orientation_and_profile() {
    let info = read(ORIENTED_TIFF).probe().expect("TIFF probes");
    assert_eq!(
        (info.width, info.height, info.orientation),
        (48, 64, Orientation::Rotate90)
    );
    let picture = read(ORIENTED_TIFF).decode().expect("TIFF decodes");
    assert_eq!(picture.dimensions(), (48, 64));
    assert_eq!(picture.icc_profile(), Some(PROFILE));
    corners(&picture, [BLUE, RED, HALF_WHITE, GREEN], 0.0);
}

#[test]
fn half_float_exr_decodes_to_float() {
    let picture = read(HALF_EXR).decode().expect("EXR decodes");
    assert_eq!(
        (picture.dimensions(), picture.color()),
        ((64, 48), ColorType::Rgba32F)
    );
    corners(&picture, [RED, GREEN, BLUE, HALF_WHITE], 0.01);
}

#[test]
fn jpeg_xl_keeps_depth_alpha_and_colour() {
    let white = [1.0, 1.0, 1.0, 1.0];
    let cases: [(&[u8], ColorType, Corners, f32); 3] = [
        (
            LOSSLESS_JXL,
            ColorType::Rgba8,
            [RED, GREEN, BLUE, HALF_WHITE],
            0.0,
        ),
        (LOSSY_JXL, ColorType::Rgb8, [RED, GREEN, BLUE, white], LOSSY),
        (
            FLOAT_JXL,
            ColorType::Rgb32F,
            [[2.0, 0.0, 0.0, 1.0], GREEN, BLUE, [0.5, 0.5, 0.5, 1.0]],
            0.0,
        ),
    ];
    for (bytes, color, expected, tolerance) in cases {
        let info = read(bytes).probe().expect("JPEG XL probes");
        assert_eq!((info.width, info.height, info.color), (64, 48, color));
        let picture = read(bytes).decode().expect("JPEG XL decodes");
        assert_eq!((picture.dimensions(), picture.color()), ((64, 48), color));
        corners(&picture, expected, tolerance.max(1e-6));
    }

    let linear = read(FLOAT_JXL).decode().expect("float JPEG XL decodes");
    let profile = linear
        .icc_profile()
        .expect("linear light carries a profile");
    assert_eq!(&profile[36..40], b"acsp");
    assert_eq!(&profile[16..20], b"RGB ");
    let srgb = read(LOSSLESS_JXL).decode().expect("JPEG XL decodes");
    assert_eq!(srgb.icc_profile(), None);

    let deep = read(DEEP_JXL).decode().expect("16-bit JPEG XL decodes");
    assert_eq!(
        (deep.dimensions(), deep.color()),
        ((128, 128), ColorType::Rgb16)
    );
    for x in [0, 1, 64, 127] {
        let red = (x * 65535 / 127) as f32 / 65535.0;
        let expected = [red, 0.0, 32768.0 / 65535.0, 1.0];
        near(&deep, x, 0, expected, 1e-6);
    }
}

#[test]
fn jpeg_xl_container_keeps_orientation_profile_and_premultiplied_alpha() {
    let info = read(ORIENTED_JXL)
        .probe()
        .expect("JPEG XL container probes");
    assert_eq!(
        (info.width, info.height, info.color, info.orientation),
        (
            48,
            64,
            ColorType::Rgba8,
            Orientation::Rotate90FlipHorizontal
        )
    );
    let picture = read(ORIENTED_JXL)
        .decode()
        .expect("JPEG XL container decodes");
    assert_eq!(
        (picture.dimensions(), picture.color()),
        ((48, 64), ColorType::Rgba8)
    );
    assert_eq!(picture.icc_profile(), Some(GAMMA_PROFILE));
    corners(&picture, [RED, BLUE, GREEN, HALF_WHITE], 0.0);
}

#[test]
fn animated_jpeg_xl_keeps_frame_timing_loops_and_alpha() {
    let animation = read(ANIMATED_JXL).animation().expect("animation decodes");
    let delays: Vec<Duration> = animation
        .frames()
        .iter()
        .map(|frame| frame.delay())
        .collect();
    assert_eq!(delays, [100, 200, 300].map(Duration::from_millis));
    assert_eq!(animation.loops(), Loops::Finite(3));
    let frames = animation.frames();
    near(frames[0].image(), 10, 10, RED, 0.0);
    near(
        frames[1].image(),
        10,
        10,
        [0.0, 1.0, 0.0, 128.0 / 255.0],
        0.0,
    );
    near(frames[2].image(), 10, 10, BLUE, 0.0);

    let still = read(ANIMATED_JXL).decode().expect("first picture decodes");
    near(&still, 10, 10, RED, 0.0);
}

#[test]
fn jpeg_xl_writes_losslessly_at_full_quality() {
    let deep = deep_gradient(32, 24);
    let pictures = [
        gradient(32, 24),
        gradient(32, 24).convert(ColorType::La8),
        gradient(32, 24).convert(ColorType::Rgb8),
        deep.clone(),
        deep.convert(ColorType::Rgba32F),
    ];
    for picture in pictures {
        let bytes = picture
            .write(Format::JpegXl)
            .quality(100)
            .to_bytes()
            .unwrap_or_else(|error| panic!("{:?}: {error}", picture.color()));
        assert_eq!(Format::detect(&bytes), Some(Format::JpegXl));
        let decoded = read(&bytes).decode().expect("written JPEG XL decodes");
        assert_eq!(decoded.color(), picture.color());
        assert_eq!(
            decoded.as_bytes(),
            picture.as_bytes(),
            "{:?}",
            picture.color()
        );
        assert_eq!(decoded.icc_profile(), None);
    }
    let profiled = gradient(32, 24).with_icc_profile(Some(GAMMA_PROFILE.into()));
    let bytes = profiled
        .write(Format::JpegXl)
        .quality(100)
        .to_bytes()
        .expect("profiled JPEG XL encodes");
    let decoded = read(&bytes).decode().expect("profiled JPEG XL decodes");
    assert_eq!(decoded.icc_profile(), Some(GAMMA_PROFILE));
}

#[test]
fn jpeg_xl_headers_are_admitted_before_decoding() {
    let info = read(HUGE_JXL).probe().expect("header probes");
    assert_eq!((info.width, info.height), (1 << 30, 1 << 30));
    let decoded = read(HUGE_JXL).decode();
    assert!(
        matches!(decoded, Err(Error::LimitsExceeded(_))),
        "{decoded:?}"
    );
}

#[test]
fn content_detection_names_ffmpeg_formats() {
    let cases = [
        (GRID_AVIF, Format::Avif),
        (ANIMATED_AVIF, Format::Avif),
        (PHOTO_HEIC, Format::Heif),
        (TILES_HEIC, Format::Heif),
        (PICTURE_JP2, Format::Jpeg2000),
        (DEEP_J2K, Format::Jpeg2000),
        (ORIENTED_TIFF, Format::Tiff),
        (HALF_EXR, Format::OpenExr),
        (LOSSLESS_JXL, Format::JpegXl),
        (DEEP_JXL, Format::JpegXl),
        (ORIENTED_JXL, Format::JpegXl),
    ];
    for (bytes, format) in cases {
        assert_eq!(Format::detect(bytes), Some(format));
    }
    assert_eq!(Format::from_extension("HEIC"), Some(Format::Heif));
    assert_eq!(Format::from_mime_type("image/heic"), Some(Format::Heif));
    assert_eq!(Format::from_extension("j2k"), Some(Format::Jpeg2000));
    assert_eq!(Format::from_extension("JXL"), Some(Format::JpegXl));
    assert_eq!(Format::from_mime_type("image/jxl"), Some(Format::JpegXl));
}

#[test]
fn ffmpeg_formats_refuse_pictures_beyond_the_limits() {
    for bytes in [
        GRID_AVIF,
        TILES_HEIC,
        PICTURE_JP2,
        ORIENTED_TIFF,
        HALF_EXR,
        ANIMATED_AVIF,
        LOSSLESS_JXL,
        ORIENTED_JXL,
        ANIMATED_JXL,
    ] {
        let narrow = read(bytes).max_dimensions(16, 16).decode();
        assert!(
            matches!(narrow, Err(Error::LimitsExceeded(_))),
            "{narrow:?}"
        );
        let small = read(bytes).max_alloc(1024).decode();
        assert!(matches!(small, Err(Error::LimitsExceeded(_))), "{small:?}");
    }
    for bytes in [ANIMATED_AVIF, ANIMATED_JXL] {
        let animation = read(bytes).max_alloc(1024).animation();
        assert!(
            matches!(animation, Err(Error::LimitsExceeded(_))),
            "{animation:?}"
        );
    }
}

#[test]
fn damaged_ffmpeg_files_fail_without_panicking() {
    let fixtures = [
        (GRID_AVIF, Format::Avif),
        (ORIENTED_AVIF, Format::Avif),
        (ANIMATED_AVIF, Format::Avif),
        (PHOTO_HEIC, Format::Heif),
        (TILES_HEIC, Format::Heif),
        (PICTURE_JP2, Format::Jpeg2000),
        (DEEP_J2K, Format::Jpeg2000),
        (ORIENTED_TIFF, Format::Tiff),
        (HALF_EXR, Format::OpenExr),
        (LOSSLESS_JXL, Format::JpegXl),
        (LOSSY_JXL, Format::JpegXl),
        (DEEP_JXL, Format::JpegXl),
        (FLOAT_JXL, Format::JpegXl),
        (ORIENTED_JXL, Format::JpegXl),
        (ANIMATED_JXL, Format::JpegXl),
    ];
    for (bytes, format) in fixtures {
        for cut in [12, 40, bytes.len() / 3] {
            let truncated = read(&bytes[..cut]).format(format).animation();
            assert!(truncated.is_err(), "{format} cut at {cut} decoded");
        }
        for at in (16..bytes.len()).step_by(bytes.len() / 24 + 1) {
            let mut flipped = bytes.to_vec();
            flipped[at] ^= 0x5A;
            let _ = read(&flipped).format(format).decode();
            let _ = read(&flipped).format(format).animation();
            let _ = read(&flipped).format(format).probe();
        }
    }
}
