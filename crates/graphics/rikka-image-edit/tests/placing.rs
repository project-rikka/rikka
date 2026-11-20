use crate::support::{at, close, developed, flat, picture};
use rikka_image_edit::{Blend, Place, Profile, Recipe};

fn moved(x: f32, y: f32) -> [[f32; 3]; 3] {
    [[1.0, 0.0, x], [0.0, 1.0, y], [0.0, 0.0, 1.0]]
}

#[test]
fn placement_follows_matrix() {
    let white = flat(20, 20, [255, 255, 255, 255]);
    let red = flat(4, 4, [255, 0, 0, 255]);

    let placed = developed(&white, Recipe::new().then(Place::new(red, moved(5.0, 6.0))));

    assert_eq!(at(&placed, 5, 6), [255, 0, 0, 255]);
    assert_eq!(at(&placed, 8, 9), [255, 0, 0, 255]);
    assert_eq!(at(&placed, 4, 6), [255, 255, 255, 255]);
    assert_eq!(at(&placed, 9, 9), [255, 255, 255, 255]);
    assert_eq!(at(&placed, 5, 10), [255, 255, 255, 255]);
}

#[test]
fn negative_scale_mirrors() {
    let white = flat(10, 4, [255, 255, 255, 255]);
    let halves = picture(10, 4, |x, _| match x < 5 {
        true => [255, 0, 0, 255],
        false => [0, 0, 255, 255],
    });

    let mirrored = developed(
        &white,
        Recipe::new().then(Place::new(
            halves,
            [[-1.0, 0.0, 10.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        )),
    );

    assert_eq!(at(&mirrored, 1, 1), [0, 0, 255, 255]);
    assert_eq!(at(&mirrored, 8, 1), [255, 0, 0, 255]);
}

#[test]
fn downscaled_pattern_averages_without_aliasing() {
    let black = flat(40, 40, [0, 0, 0, 255]);
    let checks = picture(256, 256, |x, y| match (x + y) % 2 {
        0 => [255, 255, 255, 255],
        _ => [0, 0, 0, 255],
    });
    let eighth = [[0.125, 0.0, 4.0], [0.0, 0.125, 4.0], [0.0, 0.0, 1.0]];

    let placed = developed(&black, Recipe::new().then(Place::new(checks, eighth)));

    let values: Vec<u8> = (5..35)
        .flat_map(|y| (5..35).map(move |x| (x, y)))
        .map(|(x, y)| at(&placed, x, y)[0])
        .collect();
    let low = *values.iter().min().unwrap();
    let high = *values.iter().max().unwrap();
    assert!(high - low <= 6, "{low}..{high}");
    assert!(close(low, 188, 6), "half-coverage average is {low}");
}

#[test]
fn placement_applies_opacity_and_blend() {
    let grey = flat(8, 8, [128, 128, 128, 255]);
    let white = flat(8, 8, [255, 255, 255, 255]);

    let faded = developed(
        &grey,
        Recipe::new().then(Place::new(white.clone(), moved(0.0, 0.0)).opacity(0.5)),
    );
    assert!(close(at(&faded, 3, 3)[0], 191, 2), "{:?}", at(&faded, 3, 3));

    let multiplied = developed(
        &grey,
        Recipe::new().then(Place::new(grey.clone(), moved(0.0, 0.0)).blend(Blend::Multiply)),
    );
    assert!(
        close(at(&multiplied, 3, 3)[0], 64, 1),
        "{:?}",
        at(&multiplied, 3, 3)
    );

    let by_white = developed(
        &grey,
        Recipe::new().then(Place::new(white, moved(0.0, 0.0)).blend(Blend::Multiply)),
    );
    assert_eq!(at(&by_white, 3, 3)[0], 128);
}

#[test]
fn placement_converts_to_frame_profile() {
    let frame = flat(6, 6, [0, 0, 0, 255]).with_profile(Some(Profile::display_p3()));
    let red = flat(6, 6, [255, 0, 0, 255]);

    let placed = developed(&frame, Recipe::new().then(Place::new(red, moved(0.0, 0.0))));

    let [r, g, b, _] = at(&placed, 3, 3);
    assert!(r < 245 && g > 30 && b > 15, "{r} {g} {b}");
}

#[test]
fn rejects_singular_placement() {
    let white = flat(4, 4, [255, 255, 255, 255]);
    let flattened = Place::new(
        white.clone(),
        [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
    );
    assert!(white.develop(&Recipe::new().then(flattened)).is_err());
}

#[test]
fn replacing_placement_clears_frame() {
    let white = flat(20, 20, [255, 255, 255, 255]);
    let red = flat(4, 4, [255, 0, 0, 255]);

    let placed = developed(
        &white,
        Recipe::new().then(Place::new(red, moved(5.0, 6.0)).replacing()),
    );

    assert_eq!(at(&placed, 6, 7), [255, 0, 0, 255]);
    assert_eq!(at(&placed, 1, 1)[3], 0);
    assert_eq!(at(&placed, 15, 15)[3], 0);
}

#[test]
fn mirrored_placement_reflects_beyond_edges() {
    let white = flat(20, 4, [255, 255, 255, 255]);
    let halves = picture(10, 4, |x, _| match x < 5 {
        true => [255, 0, 0, 255],
        false => [0, 0, 255, 255],
    });

    let placed = developed(
        &white,
        Recipe::new().then(Place::new(halves, moved(5.0, 0.0)).mirrored()),
    );

    assert_eq!(at(&placed, 1, 1), [255, 0, 0, 255]);
    assert_eq!(at(&placed, 7, 1), [255, 0, 0, 255]);
    assert_eq!(at(&placed, 12, 1), [0, 0, 255, 255]);
    assert_eq!(at(&placed, 19, 1), [0, 0, 255, 255]);
}
