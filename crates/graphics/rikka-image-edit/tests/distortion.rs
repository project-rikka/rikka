use crate::support::{at, developed, flat, picture, rgba};
use rikka_image_edit::{Displace, FractalNoise, LensDistortion, Recipe};

fn column(width: u32, height: u32, x: u32) -> rikka_image_edit::Picture {
    picture(width, height, |at, _| match at == x {
        true => [255, 255, 255, 255],
        false => [0, 0, 0, 255],
    })
}

fn brightest(picture: &rikka_image_edit::Picture, y: u32) -> u32 {
    (0..picture.width())
        .max_by_key(|x| at(picture, *x, y)[0])
        .unwrap_or(0)
}

#[test]
fn fractal_noise_is_seeded_and_spread() {
    let canvas = flat(48, 48, [0, 0, 0, 0]);
    let noise = |seed| {
        developed(
            &canvas,
            Recipe::new().then(FractalNoise::new(8.0).seed(seed)),
        )
    };

    let first = rgba(&noise(3));
    assert_eq!(first, rgba(&noise(3)));
    assert_ne!(first, rgba(&noise(4)));
    let values: Vec<u8> = first.chunks(4).map(|pixel| pixel[0]).collect();
    let low = *values.iter().min().unwrap();
    let high = *values.iter().max().unwrap();
    assert!(low < 100 && high > 156, "{low}..{high}");
    assert!(first.chunks(4).all(|pixel| pixel[3] == 255));
    assert!(first.chunks(4).all(|pixel| pixel[0] == pixel[1]));

    let colored = rgba(&developed(
        &canvas,
        Recipe::new().then(FractalNoise::new(8.0).seed(3).colored()),
    ));
    assert!(colored.chunks(4).any(|pixel| pixel[0] != pixel[1]));
}

#[test]
fn displace_neutral_and_full_maps() {
    let line = column(24, 6, 12);
    let neutral = flat(4, 4, [128, 128, 0, 255]);
    let rightward = flat(4, 4, [255, 128, 0, 255]);

    let unshifted = developed(&line, Recipe::new().then(Displace::new(neutral, 4.0, 4.0)));
    let shifted = developed(
        &line,
        Recipe::new().then(Displace::new(rightward, 4.0, 0.0)),
    );

    assert_eq!(brightest(&unshifted, 3), 12);
    assert_eq!(brightest(&shifted, 3), 8);
}

#[test]
fn barrel_distortion_keeps_center_pulls_edges() {
    let line = column(41, 41, 36);
    let centre = column(41, 41, 20);

    let bent = developed(&line, Recipe::new().then(LensDistortion::new(50.0)));
    let centered = developed(&centre, Recipe::new().then(LensDistortion::new(50.0)));

    assert_eq!(brightest(&centered, 20), 20);
    let moved = brightest(&bent, 20);
    assert!((26..36).contains(&moved), "edge line at {moved}");
}
