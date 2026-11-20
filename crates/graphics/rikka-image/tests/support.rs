use rikka_image::{ColorType, Image};
use rikka_types::Color;

pub(crate) fn gradient(width: u32, height: u32) -> Image {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.extend_from_slice(&[
                (x * 255 / width.max(1)) as u8,
                (y * 255 / height.max(1)) as u8,
                ((x + y) * 127 / (width + height).max(1)) as u8,
                (128 + (x % 128)) as u8,
            ]);
        }
    }
    Image::from_rgba8(width, height, pixels).expect("buffer matches dimensions")
}

pub(crate) fn opaque_gradient(width: u32, height: u32) -> Image {
    gradient(width, height).convert(ColorType::Rgb8)
}

pub(crate) fn solid(width: u32, height: u32, color: Color, layout: ColorType) -> Image {
    Image::filled(width, height, layout, color).expect("valid size")
}

pub(crate) fn close(left: Color, right: Color) -> bool {
    let tolerance = 2.0 / 255.0;
    (left.r - right.r).abs() <= tolerance
        && (left.g - right.g).abs() <= tolerance
        && (left.b - right.b).abs() <= tolerance
        && (left.a - right.a).abs() <= tolerance
}

pub(crate) fn every_pixel_close(image: &Image, color: Color) {
    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.pixel(x, y).expect("pixel in bounds");
            assert!(close(pixel, color), "pixel {x},{y} is {pixel:?}");
        }
    }
}
