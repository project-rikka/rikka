use crate::support::{at, developed, picture};
use rikka_image_edit::{DropShadow, Outline, Recipe};
use rikka_types::Color;

fn square_on_nothing(side: u32, from: u32, to: u32, texel: [u8; 4]) -> rikka_image_edit::Picture {
    picture(side, side, move |x, y| {
        match (from..to).contains(&x) && (from..to).contains(&y) {
            true => texel,
            false => [0, 0, 0, 0],
        }
    })
}

#[test]
fn drop_shadow_follows_angle() {
    let square = square_on_nothing(30, 10, 20, [255, 255, 255, 255]);
    let shadow = DropShadow::new(Color::BLACK)
        .angle(-45.0)
        .distance(4.0f32.hypot(4.0))
        .blur(0.0)
        .opacity(1.0);

    let cast = developed(&square, Recipe::new().then(shadow));

    assert_eq!(at(&cast, 15, 15), [255, 255, 255, 255]);
    assert_eq!(at(&cast, 22, 22), [0, 0, 0, 255]);
    assert_eq!(at(&cast, 21, 14), [0, 0, 0, 255]);
    assert_eq!(at(&cast, 8, 8)[3], 0);
    assert_eq!(at(&cast, 25, 25)[3], 0);
}

#[test]
fn outline_grows_around_shape() {
    let square = square_on_nothing(30, 12, 18, [255, 0, 0, 255]);

    let outlined = developed(
        &square,
        Recipe::new().then(Outline::new(Color::rgb(0.0, 0.0, 1.0)).width(3.0)),
    );

    assert_eq!(at(&outlined, 15, 15), [255, 0, 0, 255]);
    assert_eq!(at(&outlined, 10, 15), [0, 0, 255, 255]);
    assert_eq!(at(&outlined, 9, 15), [0, 0, 255, 255]);
    assert_eq!(at(&outlined, 8, 15)[3], 0);
    assert_eq!(at(&outlined, 20, 15), [0, 0, 255, 255]);
    assert_eq!(at(&outlined, 21, 15)[3], 0);
    assert_eq!(at(&outlined, 9, 9)[3], 0);
    assert_eq!(at(&outlined, 10, 10), [0, 0, 255, 255]);
}
