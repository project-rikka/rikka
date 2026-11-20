use crate::support::{at, close, developed, flat, picture};
use rikka_image::Anchor;
use rikka_image_edit::{
    CanvasSize, Crop, QuarterTurn, Recipe, Resampling, Resize, Rotate, Transform,
};
use rikka_types::Color;

fn labelled() -> rikka_image_edit::Picture {
    picture(4, 3, |x, y| [(x * 60) as u8, (y * 100) as u8, 7, 255])
}

#[test]
fn quarter_turns_move_corners() {
    let turned = developed(&labelled(), Recipe::new().then(QuarterTurn::Clockwise));
    assert_eq!(turned.dimensions(), (3, 4));
    assert_eq!(
        at(&turned, 2, 0),
        at(&labelled(), 0, 0),
        "top left goes to top right"
    );
    let back = developed(&turned, Recipe::new().then(QuarterTurn::CounterClockwise));
    assert_eq!(
        crate::support::rgba(&back),
        crate::support::rgba(&labelled())
    );
    let half = developed(&labelled(), Recipe::new().then(QuarterTurn::Half));
    assert_eq!(at(&half, 0, 0), at(&labelled(), 3, 2));
}

#[test]
fn crop_keeps_exact_rectangle() {
    let cropped = developed(&labelled(), Recipe::new().then(Crop::new(1, 1, 2, 2)));
    assert_eq!(cropped.dimensions(), (2, 2));
    assert_eq!(at(&cropped, 0, 0), at(&labelled(), 1, 1));
    assert_eq!(at(&cropped, 1, 1), at(&labelled(), 2, 2));
}

#[test]
fn canvas_grow_fills_around_anchor() {
    let grown = developed(
        &labelled(),
        Recipe::new().then(
            CanvasSize::new(6, 5)
                .anchor(Anchor::BottomRight)
                .fill(Color::rgb(0.0, 0.0, 1.0)),
        ),
    );
    assert_eq!(grown.dimensions(), (6, 5));
    assert_eq!(at(&grown, 0, 0), [0, 0, 255, 255]);
    assert_eq!(at(&grown, 5, 4), at(&labelled(), 3, 2));
    let clear = developed(&labelled(), Recipe::new().then(CanvasSize::new(6, 5)));
    assert!(clear.has_alpha());
    assert_eq!(at(&clear, 0, 0)[3], 0);
}

#[test]
fn halving_checkerboard_averages() {
    let checker = picture(16, 16, |x, y| match (x + y) % 2 {
        0 => [255, 255, 255, 255],
        _ => [0, 0, 0, 255],
    });
    let halved = developed(
        &checker,
        Recipe::new().then(Resize::new(8, 8).resampling(Resampling::Bilinear)),
    );
    let expected = ((0.5f32.powf(1.0 / 2.4) * 1.055 - 0.055) * 255.0).round() as u8;
    assert!(
        close(at(&halved, 4, 4)[0], expected, 3),
        "averaged in linear light"
    );
    let nearest = developed(
        &checker,
        Recipe::new().then(Resize::new(32, 32).resampling(Resampling::Nearest)),
    );
    assert_eq!(at(&nearest, 0, 0), [255, 255, 255, 255]);
    assert_eq!(at(&nearest, 2, 0), [0, 0, 0, 255]);
}

#[test]
fn rotation_grows_canvas_to_fit() {
    let square = flat(20, 10, [255, 0, 0, 255]);
    let rotated = developed(&square, Recipe::new().then(Rotate::new(90.0)));
    assert_eq!(rotated.dimensions(), (10, 20));
    assert_eq!(at(&rotated, 5, 10), [255, 0, 0, 255]);
    let tilted = developed(&square, Recipe::new().then(Rotate::new(45.0)));
    assert_eq!(tilted.dimensions(), (21, 21));
    assert_eq!(at(&tilted, 0, 0)[3], 0, "new corners transparent");
    assert_eq!(at(&tilted, 10, 10), [255, 0, 0, 255]);
    let rotated = developed(&square, Recipe::new().then(Rotate::new(45.0).expand(false)));
    assert_eq!(rotated.dimensions(), (20, 10));
}

#[test]
fn perspective_maps_corners() {
    let source = picture(20, 20, |x, _| match x < 10 {
        true => [255, 0, 0, 255],
        false => [0, 0, 255, 255],
    });
    let identity = Transform::corners(
        (20, 20),
        [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
        20,
        20,
    )
    .unwrap();
    let same = developed(&source, Recipe::new().then(identity));
    assert!(close(at(&same, 3, 3)[0], 255, 1) && close(at(&same, 16, 3)[2], 255, 1));
    let squeezed = Transform::corners(
        (20, 20),
        [(5.0, 0.0), (15.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
        20,
        20,
    )
    .unwrap();
    let narrowed = developed(&source, Recipe::new().then(squeezed));
    assert_eq!(
        at(&narrowed, 1, 0)[3],
        0,
        "outside the quadrilateral is clear"
    );
    assert!(close(at(&narrowed, 10, 19)[3], 255, 1));
}
