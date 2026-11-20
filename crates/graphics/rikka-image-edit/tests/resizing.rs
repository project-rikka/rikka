use crate::support::{imaging, picture};
use rikka_image::{ColorType, Image};
use rikka_image_edit::{Fill, Mask, Picture, Recipe, Resampling, Resize, Step};
use rikka_types::Color;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;

fn shaded() -> Picture {
    picture(WIDTH, HEIGHT, |x, y| {
        [
            (x * 7 % 256) as u8,
            (y * 5 % 256) as u8,
            ((x + y) % 256) as u8,
            (96 + x % 150) as u8,
        ]
    })
}

fn plain(color: ColorType) -> Picture {
    let image =
        Image::filled(WIDTH, HEIGHT, color, Color::rgb(0.3, 0.6, 0.45)).expect("valid image");
    imaging().picture(&image).expect("valid picture")
}

fn exact(picture: &Picture) -> Vec<u8> {
    picture
        .to_image()
        .expect("picture downloads")
        .as_bytes()
        .to_vec()
}

fn resized(picture: &Picture, [width, height]: [u32; 2], resampling: Resampling) -> Picture {
    picture
        .develop(&Recipe::new().then(Resize::new(width, height).resampling(resampling)))
        .expect("picture resizes")
}

fn texel(x: u32, y: u32) -> [u8; 4] {
    let seed = x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(2_246_822_519);
    let mixed = seed ^ (seed >> 13);
    [
        mixed as u8,
        (mixed >> 8) as u8,
        (mixed >> 16) as u8,
        128 | (mixed >> 24) as u8,
    ]
}

#[test]
fn incremental_resize_matches_full_resize() {
    let cases: [([u32; 2], Resampling, Mask); 7] = [
        (
            [160, 100],
            Resampling::BicubicSmoother,
            Mask::ellipse(150.0, 90.0, 20.0, 14.0),
        ),
        (
            [107, 67],
            Resampling::Lanczos,
            Mask::ellipse(6.0, 4.0, 18.0, 11.0),
        ),
        (
            [640, 400],
            Resampling::Bicubic,
            Mask::ellipse(310.0, 195.0, 25.0, 12.0),
        ),
        (
            [320, 50],
            Resampling::Bilinear,
            Mask::ellipse(40.0, 120.0, 9.0, 30.0),
        ),
        (
            [97, 200],
            Resampling::Nearest,
            Mask::ellipse(200.0, 20.0, 15.0, 15.0),
        ),
        (
            [12, 8],
            Resampling::BicubicSmoother,
            Mask::ellipse(100.0, 100.0, 4.0, 4.0),
        ),
        (
            [320, 200],
            Resampling::BicubicSmoother,
            Mask::ellipse(260.0, 60.0, 30.0, 8.0),
        ),
    ];
    let fill = Fill::new(Color::rgba(0.9, 0.2, 0.1, 0.7));
    for before in [shaded(), plain(ColorType::Rgb16), plain(ColorType::L8)] {
        for (extent, resampling, mask) in &cases {
            let after = before
                .develop(&Recipe::new().then(Step::new(fill).masked(mask.clone())))
                .expect("fill develops");
            let changed = after
                .changed_area(&before)
                .expect("pictures compare")
                .expect("fill changes pixels");
            let earlier = resized(&before, *extent, *resampling);
            let updated = after
                .resized_since(&earlier, changed, *resampling)
                .expect("resize updates");
            let reference = resized(&after, *extent, *resampling);
            assert_eq!(updated.dimensions(), reference.dimensions());
            assert!(
                exact(&updated) == exact(&reference),
                "{resampling:?} to {extent:?} of a {:?} {:?} picture changed in {changed:?} differs from resizing it whole",
                before.depth(),
                before.mode()
            );
        }
    }
}

#[test]
fn incremental_resize_keeps_earlier_result() {
    let before = picture(1300, 900, texel);
    let fill = Fill::new(Color::rgba(0.2, 0.3, 0.9, 0.6));
    for (extent, mask) in [
        ([1300, 900], Mask::ellipse(490.0, 480.0, 60.0, 50.0)),
        ([650, 450], Mask::ellipse(1000.0, 500.0, 40.0, 40.0)),
        ([1950, 1350], Mask::ellipse(500.0, 500.0, 30.0, 30.0)),
    ] {
        let after = before
            .develop(&Recipe::new().then(Step::new(fill).masked(mask)))
            .expect("fill develops");
        let changed = after
            .changed_area(&before)
            .expect("pictures compare")
            .expect("fill changes pixels");
        let earlier = resized(&before, extent, Resampling::BicubicSmoother);
        let snapshot = exact(&earlier);
        let updated = after
            .resized_since(&earlier, changed, Resampling::BicubicSmoother)
            .expect("resize updates");
        let reference = resized(&after, extent, Resampling::BicubicSmoother);
        assert!(exact(&updated) == exact(&reference), "{extent:?}");
        assert!(
            exact(&earlier) == snapshot,
            "{extent:?} changed the earlier resize"
        );
    }
}

#[test]
fn rejects_outside_update_and_keeps_empty_one() {
    let before = shaded();
    let earlier = resized(&before, [80, 50], Resampling::Lanczos);
    assert!(
        before
            .resized_since(&earlier, [300, 190, 30, 20], Resampling::Lanczos)
            .is_err()
    );
    let unchanged = before
        .resized_since(&earlier, [10, 10, 0, 0], Resampling::Lanczos)
        .expect("no change");
    assert!(exact(&unchanged) == exact(&earlier));
}

#[test]
fn pieced_resize_preserves_repeating_pattern() {
    let cases: [([u32; 2], [u32; 2], u32, u32); 2] = [
        ([2400, 1000], [1200, 500], 16, 4),
        ([2048, 1536], [32, 24], 256, 3),
    ];
    for (source, extent, period, margin) in cases {
        let repeating = picture(source[0], source[1], |x, y| texel(x % period, y % period));
        let reduced = resized(&repeating, extent, Resampling::BicubicSmoother);
        let pixels = exact(&reduced);
        let step = period * extent[0] / source[0];
        let at = |x: u32, y: u32| {
            let index = ((y * extent[0] + x) * 4) as usize;
            &pixels[index..index + 4]
        };
        for y in margin..extent[1] - margin {
            for x in margin..extent[0] - margin {
                if x >= margin + step {
                    assert_eq!(
                        at(x, y),
                        at(x - step, y),
                        "column {x} row {y} of {extent:?}"
                    );
                }
                if y >= margin + step {
                    assert_eq!(
                        at(x, y),
                        at(x, y - step),
                        "column {x} row {y} of {extent:?}"
                    );
                }
            }
        }
    }
}
