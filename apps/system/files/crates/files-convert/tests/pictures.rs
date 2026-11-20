use std::time::Duration;

use files_convert::{PictureFormat, Scale, Target};
use rikka_image::{ColorScheme, ColorType, Frame, GifWriter, Image, Loops, read};
use rikka_types::Color;

use crate::support::{converted_in, names, succeeded};

fn picture(width: u32, height: u32) -> Image {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[(x * 7) as u8, (y * 5) as u8, 90, 255]);
        }
    }
    Image::from_rgba8(width, height, pixels).expect("build picture")
}

#[test]
fn picture_formats_convert_and_decode() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("photo.png");
    picture(40, 30).save(&source).expect("write source");

    for format in PictureFormat::ALL {
        if format == PictureFormat::Png {
            continue;
        }
        succeeded(&[&source], Target::Picture(format), "");
        let written = folder.path().join(format!("photo.{}", format.extension()));
        let bytes = std::fs::read(&written).unwrap_or_else(|_| panic!("{format:?} output missing"));
        let decoded = read(bytes)
            .decode()
            .unwrap_or_else(|error| panic!("{format:?}: {error}"));
        assert_eq!(decoded.dimensions(), (40, 30), "{format:?}");
    }
}

#[test]
fn conversion_never_overwrites() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("photo.png");
    picture(8, 8).save(&source).expect("write source");
    std::fs::write(folder.path().join("photo.jpg"), b"keep me").expect("write bystander file");

    succeeded(&[&source], Target::Picture(PictureFormat::Jpeg), "");
    succeeded(&[&source], Target::Shrink(Scale::Half), " (small)");
    succeeded(&[&source], Target::Shrink(Scale::Half), " (small)");

    assert_eq!(
        std::fs::read(folder.path().join("photo.jpg")).expect("read bystander file"),
        b"keep me"
    );
    assert_eq!(
        names(folder.path()),
        [
            "photo (2).jpg",
            "photo (small) (2).png",
            "photo (small).png",
            "photo.jpg",
            "photo.png"
        ]
    );
    let small = read(folder.path().join("photo (small).png"))
        .decode()
        .expect("decode shrunk picture");
    assert_eq!(small.dimensions(), (4, 4));
}

#[test]
fn shrink_keeps_aspect_never_enlarges() {
    let folder = tempfile::tempdir().unwrap();
    let wide = folder.path().join("wide.png");
    let tiny = folder.path().join("tiny.png");
    picture(300, 100).save(&wide).expect("save wide picture");
    picture(20, 10).save(&tiny).expect("save tiny picture");

    succeeded(
        &[&wide, &tiny],
        Target::Shrink(Scale::LongestSide(150)),
        "-150",
    );

    let dimensions = |name: &str| {
        read(folder.path().join(name))
            .decode()
            .expect("decode shrunk picture")
            .dimensions()
    };
    assert_eq!(dimensions("wide-150.png"), (150, 50));
    assert_eq!(dimensions("tiny-150.png"), (20, 10));
}

#[test]
fn jpeg_flattens_alpha_onto_white() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("clear.png");
    Image::filled(4, 4, ColorType::Rgba8, Color::rgba(0.0, 0.0, 0.0, 0.0))
        .expect("build transparent picture")
        .save(&source)
        .expect("write source");

    succeeded(&[&source], Target::Picture(PictureFormat::Jpeg), "");

    let flattened = read(folder.path().join("clear.jpg"))
        .decode()
        .expect("decode jpeg");
    let corner = flattened.pixel(0, 0).expect("pixel in bounds");
    assert!(
        corner.r > 0.95 && corner.g > 0.95 && corner.b > 0.95,
        "{corner:?}"
    );
}

#[test]
fn animation_survives_shrink_and_gif() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("blink.gif");
    let mut writer = GifWriter::create(&source, 20, 20, Loops::Infinite).expect("create gif");
    for color in [Color::WHITE, Color::BLACK, Color::WHITE] {
        let still = Image::filled(20, 20, ColorType::Rgb8, color).expect("build frame");
        writer
            .push(Frame::new(still, Duration::from_millis(200)))
            .expect("build frame");
    }
    writer.finish().expect("finish gif");

    succeeded(&[&source], Target::Shrink(Scale::Half), " (half)");

    let shrunk = read(folder.path().join("blink (half).gif"))
        .animation()
        .expect("decode shrunk animation");
    assert_eq!(shrunk.frame_count(), 3);
    assert_eq!(shrunk.dimensions(), (10, 10));
    assert_eq!(shrunk.frames()[1].delay(), Duration::from_millis(200));
}

#[test]
fn svg_rasterizes_at_intrinsic_size() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("logo.svg");
    std::fs::write(
        &source,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"48\" height=\"24\">\
         <rect width=\"48\" height=\"24\" fill=\"#3366ff\"/></svg>",
    )
    .expect("write drawing");

    succeeded(&[&source], Target::Picture(PictureFormat::Png), "");

    let drawn = read(folder.path().join("logo.png"))
        .decode()
        .expect("rasterize drawing");
    assert_eq!(drawn.dimensions(), (48, 24));
}

#[test]
fn svg_is_drawn_in_the_chosen_color_scheme() {
    for (scheme, shade) in [(ColorScheme::Light, 255), (ColorScheme::Dark, 0)] {
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("themed.svg");
        std::fs::write(
            &source,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\">\
             <style>rect { fill: #ffffff } \
             @media (prefers-color-scheme: dark) { rect { fill: #000000 } }</style>\
             <rect width=\"8\" height=\"8\"/></svg>",
        )
        .expect("write drawing");

        let outcome = converted_in(&[&source], Target::Picture(PictureFormat::Png), scheme, "");
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        let drawn = read(folder.path().join("themed.png"))
            .decode()
            .expect("rasterize drawing")
            .into_rgba8();
        assert_eq!(drawn[..4], [shade, shade, shade, 255], "{scheme:?}");
    }
}
