use rikka_image::{ColorType, Error, Filter, Region};
use rikka_types::Color;

use crate::support::{close, every_pixel_close, gradient, solid};

#[test]
fn crop_clips_to_image_and_rejects_outside_region() {
    let source = gradient(10, 10);

    let inside = source.crop(Region::new(2, 2, 4, 4)).expect("crops");
    assert_eq!(inside.dimensions(), (4, 4));
    assert_eq!(inside.pixel(0, 0), source.pixel(2, 2));

    let clipped = source.crop(Region::new(8, 8, 100, 100)).expect("crops");
    assert_eq!(clipped.dimensions(), (2, 2));

    let error = source
        .crop(Region::new(20, 20, 4, 4))
        .expect_err("nothing to crop");
    assert!(
        matches!(error, Error::RegionOutOfBounds { .. }),
        "{error:?}"
    );
}

#[test]
fn resize_modes_and_thumbnail_never_upscales() {
    let source = gradient(100, 50);
    let size =
        |resized: rikka_image::Result<rikka_image::Image>| resized.expect("resizes").dimensions();

    assert_eq!(
        size(source.resize_to_fit(50, 50, Filter::Lanczos3)),
        (50, 25)
    );
    assert_eq!(
        size(source.resize_exact(33, 33, Filter::Bilinear)),
        (33, 33)
    );

    let small = gradient(10, 10);
    assert_eq!(
        small.clone().thumbnail(100, 100).expect("thumbnail fits"),
        small
    );
    assert_eq!(size(small.clone().thumbnail(4, 8)), (4, 4));

    assert!(matches!(
        small.resize_exact(0, 10, Filter::Bilinear),
        Err(Error::InvalidSize { .. })
    ));
}

#[test]
fn resize_keeps_layout_and_translucent_color() {
    let translucent = Color::rgba(0.2, 0.4, 0.6, 0.5);

    let resized = solid(64, 64, translucent, ColorType::Rgba16)
        .resize_exact(16, 16, Filter::Lanczos3)
        .expect("resizes");

    assert_eq!(resized.color(), ColorType::Rgba16);
    every_pixel_close(&resized, translucent);
}

#[test]
fn overlay_blends_alpha_and_paste_replaces() {
    let red = Color::rgb(1.0, 0.0, 0.0);
    let blue_half = Color::rgba(0.0, 0.0, 1.0, 0.5);
    let top = solid(2, 2, blue_half, ColorType::Rgba8);

    let composed = solid(4, 4, red, ColorType::Rgb8).overlay(&top, 1, 1);
    assert_eq!(composed.color(), ColorType::Rgb8);
    assert!(close(
        composed.pixel(1, 1).expect("inside"),
        Color::rgb(0.5, 0.0, 0.5)
    ));
    assert!(close(composed.pixel(0, 0).expect("inside"), red));

    let mut pasted = solid(4, 4, red, ColorType::Rgba8);
    pasted.paste_in_place(&top, -1, -1);
    assert_eq!(pasted.dimensions(), (4, 4));
    assert!(close(pasted.pixel(0, 0).expect("inside"), blue_half));
    assert!(close(pasted.pixel(1, 1).expect("inside"), red));
}
