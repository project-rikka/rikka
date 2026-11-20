use crate::support::{at, close, developed, flat, picture, rgba};
use rikka_image_edit::{
    BoxBlur, Clarity, Dither, Dithering, GaussianBlur, HighPass, LensBlur, Median, MotionBlur,
    Recipe, SurfaceBlur, UnsharpMask,
};

fn dot(width: u32, height: u32) -> rikka_image_edit::Picture {
    picture(width, height, |x, y| {
        match (x, y) == (width / 2, height / 2) {
            true => [255, 255, 255, 255],
            false => [0, 0, 0, 255],
        }
    })
}

fn red_sum(picture: &rikka_image_edit::Picture) -> u64 {
    rgba(picture)
        .chunks(4)
        .map(|pixel| u64::from(pixel[0]))
        .sum()
}

#[test]
fn gaussian_blur_is_isotropic() {
    let blurred = developed(&dot(33, 33), Recipe::new().then(GaussianBlur::new(3.0)));
    let center = at(&blurred, 16, 16)[0];
    assert!(center < 255 && center > 0);
    assert_eq!(at(&blurred, 13, 16)[0], at(&blurred, 19, 16)[0]);
    assert_eq!(at(&blurred, 16, 13)[0], at(&blurred, 16, 19)[0]);
    assert_eq!(at(&blurred, 0, 0)[0], 0);
}

#[test]
fn box_blur_averages_window() {
    let stripes = picture(9, 1, |x, _| match x % 3 {
        0 => [255, 255, 255, 255],
        _ => [0, 0, 0, 255],
    });
    let averaged = developed(&stripes, Recipe::new().then(BoxBlur::new(1.0)));
    let middle = at(&averaged, 4, 0)[0];
    let expected = (((1.0f32 / 3.0).powf(1.0 / 2.4) * 1.055 - 0.055) * 255.0).round() as u8;
    assert!(close(middle, expected, 2), "{middle} ≠ {expected}");
}

#[test]
fn motion_blur_follows_direction() {
    let smeared = developed(&dot(41, 41), Recipe::new().then(MotionBlur::new(0.0, 10.0)));
    assert!(at(&smeared, 24, 20)[0] > 0, "pixel along motion");
    assert_eq!(at(&smeared, 20, 24)[0], 0, "pixel across motion");
}

#[test]
fn lens_blur_conserves_energy_and_blooms_highlights() {
    let source = dot(41, 41);
    let soft = developed(
        &source,
        Recipe::new().then(LensBlur::new(6.0).iris(6, 0.0, 0.2)),
    );
    assert!(at(&soft, 20, 20)[0] < 255);
    assert!(at(&soft, 24, 20)[0] > 0);
    let boosted = developed(
        &source,
        Recipe::new().then(LensBlur::new(6.0).specular(60.0, 0.9)),
    );
    assert!(red_sum(&boosted) > red_sum(&soft));
}

#[test]
fn surface_blur_keeps_edges_median_removes_specks() {
    let edge = picture(20, 4, |x, _| match x < 10 {
        true => [20, 20, 20, 255],
        false => [230, 230, 230, 255],
    });
    let blurred = developed(&edge, Recipe::new().then(SurfaceBlur::new(4.0, 15.0)));
    assert!(close(at(&blurred, 9, 1)[0], 20, 2) && close(at(&blurred, 10, 1)[0], 230, 2));
    let specked = picture(9, 9, |x, y| match (x, y) == (4, 4) {
        true => [255, 255, 255, 255],
        false => [40, 40, 40, 255],
    });
    let cleaned = developed(&specked, Recipe::new().then(Median::new(1.0)));
    assert!(close(at(&cleaned, 4, 4)[0], 40, 1));
}

#[test]
fn sharpening_raises_edge_contrast() {
    let edge = picture(20, 4, |x, _| match x < 10 {
        true => [80, 80, 80, 255],
        false => [170, 170, 170, 255],
    });
    let sharp = developed(&edge, Recipe::new().then(UnsharpMask::new(150.0, 2.0)));
    assert!(at(&sharp, 9, 1)[0] < 80 && at(&sharp, 10, 1)[0] > 170);
    let ignored = developed(
        &edge,
        Recipe::new().then(UnsharpMask::new(150.0, 2.0).threshold(255.0)),
    );
    assert_eq!(rgba(&ignored), rgba(&edge));
    let passed = developed(
        &flat(6, 6, [90, 200, 10, 255]),
        Recipe::new().then(HighPass::new(3.0)),
    );
    assert!(close(at(&passed, 3, 3)[0], 128, 1));
    let clearer = developed(&edge, Recipe::new().then(Clarity::new(80.0).radius(4.0)));
    assert!(at(&clearer, 10, 1)[0] > 170);
}

#[test]
fn dithering_quantizes_to_requested_levels() {
    let ramp = picture(64, 16, |x, _| {
        [(x * 4) as u8, (x * 4) as u8, (x * 4) as u8, 255]
    });
    for method in [Dithering::Ordered, Dithering::Diffusion] {
        let dithered = developed(&ramp, Recipe::new().then(Dither::new(2).method(method)));
        let pixels = rgba(&dithered);
        assert!(
            pixels
                .chunks(4)
                .all(|pixel| pixel[0] == 0 || pixel[0] == 255),
            "{method:?}"
        );
        let ones = pixels.chunks(4).filter(|pixel| pixel[0] == 255).count() as f32 / (64.0 * 16.0);
        assert!(
            (ones - 0.5).abs() < 0.1,
            "{method:?} keeps the average brightness: {ones}"
        );
    }
}
