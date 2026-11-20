use crate::support::{at, developed, flat, imaging};
use rikka_image_edit::{Mask, Profile, Recipe, Stamp, Step};

fn square(side: u32, texel: [u8; 4]) -> Vec<u8> {
    (0..side * side).flat_map(|_| texel).collect()
}

#[test]
fn stamp_composites_at_position() {
    let white = flat(20, 20, [255, 255, 255, 255]);
    let stamp = Stamp::new(4, 4, square(4, [255, 0, 0, 255]))
        .expect("valid stamp")
        .with_position(5, 6);

    let stamped = developed(&white, Recipe::new().then(stamp));

    assert_eq!(at(&stamped, 5, 6), [255, 0, 0, 255]);
    assert_eq!(at(&stamped, 8, 9), [255, 0, 0, 255]);
    assert_eq!(at(&stamped, 4, 6), [255, 255, 255, 255]);
    assert_eq!(at(&stamped, 9, 9), [255, 255, 255, 255]);
}

#[test]
fn stamp_clips_at_picture_edge() {
    let clear = flat(6, 6, [0, 0, 0, 0]);
    let stamp = Stamp::new(4, 4, square(4, [0, 255, 0, 255]))
        .expect("valid stamp")
        .with_position(-2, 4);

    let stamped = developed(&clear, Recipe::new().then(stamp));

    assert_eq!(at(&stamped, 0, 4), [0, 255, 0, 255]);
    assert_eq!(at(&stamped, 1, 5), [0, 255, 0, 255]);
    assert_eq!(at(&stamped, 2, 4)[3], 0);
    assert_eq!(at(&stamped, 0, 3)[3], 0);
}

#[test]
fn masked_stamp_stays_inside_mask() {
    let white = flat(10, 10, [255, 255, 255, 255]);
    let stamp = Stamp::new(10, 10, square(10, [0, 0, 0, 255])).expect("valid stamp");

    let stamped = developed(
        &white,
        Recipe::new().then(Step::new(stamp).masked(Mask::rectangle(0.0, 0.0, 5.0, 10.0))),
    );

    assert_eq!(at(&stamped, 2, 5), [0, 0, 0, 255]);
    assert_eq!(at(&stamped, 7, 5), [255, 255, 255, 255]);
}

#[test]
fn stamp_converts_to_picture_profile() {
    let tagged = imaging()
        .blank(
            4,
            4,
            rikka_image_edit::Depth::Eight,
            rikka_types::Color::WHITE,
        )
        .expect("valid picture")
        .with_profile(Some(Profile::adobe_rgb()));
    let stamp = Stamp::new(4, 4, square(4, [0, 200, 0, 255])).expect("valid stamp");

    let [r, _, _, _] = at(&developed(&tagged, Recipe::new().then(stamp)), 1, 1);

    assert!(r > 30, "sRGB green is less saturated in Adobe RGB: {r}");
}

#[test]
fn stamp_rejects_wrong_pixel_length() {
    assert!(Stamp::new(4, 4, vec![0; 15]).is_err());
    assert!(Stamp::new(0, 4, Vec::new()).is_err());
}
