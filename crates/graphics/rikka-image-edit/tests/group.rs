use crate::support::{at, developed, picture};
use rikka_image_edit::{GaussianBlur, Group, Recipe, Stamp};

fn square(side: u32, texel: [u8; 4]) -> Vec<u8> {
    (0..side * side).flat_map(|_| texel).collect()
}

#[test]
fn group_blurs_own_layer_and_composites_over() {
    let halves = picture(24, 24, |x, _| match x < 12 {
        true => [0, 0, 0, 255],
        false => [255, 255, 255, 255],
    });
    let stamp = Stamp::new(4, 4, square(4, [255, 0, 0, 255]))
        .expect("valid stamp")
        .with_position(14, 2);
    let inner = Recipe::new().then(stamp).then(GaussianBlur::new(2.0));

    let grouped = developed(&halves, Recipe::new().then(Group::new(inner)));

    assert_eq!(at(&grouped, 11, 20), [0, 0, 0, 255]);
    assert_eq!(at(&grouped, 12, 20), [255, 255, 255, 255]);
    let [r, g, b, a] = at(&grouped, 15, 3);
    assert!(r > 200 && g < 160 && b < 160 && a == 255, "{r} {g} {b} {a}");
    let [_, g, _, _] = at(&grouped, 19, 3);
    assert!(g < 255, "blur spreads past stamp");
}
