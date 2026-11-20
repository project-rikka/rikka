use crate::support::{at, close, developed, flat, picture, rgba};
use rikka_image_edit::{
    Blend, GaussianBlur, Invert, Mask, Preview, QuarterTurn, Recipe, Resize, Step,
};

#[test]
fn mask_confines_step() {
    let grey = flat(20, 20, [100, 100, 100, 255]);
    let inside = developed(
        &grey,
        Recipe::new().then(Step::new(Invert).masked(Mask::rectangle(5.0, 5.0, 10.0, 10.0))),
    );
    assert_eq!(at(&inside, 10, 10)[0], 155);
    assert_eq!(at(&inside, 2, 2)[0], 100);
    let outside = developed(
        &grey,
        Recipe::new()
            .then(Step::new(Invert).masked(Mask::rectangle(5.0, 5.0, 10.0, 10.0).inverted())),
    );
    assert_eq!(at(&outside, 10, 10)[0], 100);
    assert_eq!(at(&outside, 2, 2)[0], 155);
}

#[test]
fn feathered_mask_fades_edge() {
    let black = flat(40, 8, [0, 0, 0, 255]);
    let faded = developed(
        &black,
        Recipe::new()
            .then(Step::new(Invert).masked(Mask::rectangle(20.0, 0.0, 20.0, 8.0).feathered(8.0))),
    );
    let values: Vec<u8> = (10..30).map(|x| at(&faded, x, 4)[0]).collect();
    assert!(
        values.windows(2).all(|pair| pair[0] <= pair[1]),
        "{values:?}"
    );
    assert!(values[0] < 30 && *values.last().unwrap() > 225);
}

#[test]
fn opacity_and_blend_modes_mix_step() {
    let grey = flat(4, 4, [100, 100, 100, 255]);
    let half = developed(&grey, Recipe::new().then(Step::new(Invert).opacity(0.5)));
    assert!(close(at(&half, 1, 1)[0], 128, 1));
    let multiplied = developed(
        &grey,
        Recipe::new().then(Step::new(Invert).blend(Blend::Multiply)),
    );
    assert!(close(
        at(&multiplied, 1, 1)[0],
        (100.0 * 155.0 / 255.0f32).round() as u8,
        1
    ));
    let screened = developed(
        &grey,
        Recipe::new().then(Step::new(Invert).blend(Blend::Screen)),
    );
    assert!(at(&screened, 1, 1)[0] > 155);
    for blend in Blend::ALL {
        developed(
            &grey,
            Recipe::new().then(Step::new(Invert).blend(blend).opacity(0.7)),
        );
    }
}

#[test]
fn rejects_masked_reshaping_step() {
    let grey = flat(8, 6, [100, 100, 100, 255]);
    let recipe = Recipe::new().then(Step::new(Resize::new(4, 4)).opacity(0.5));
    assert!(grey.develop(&recipe).is_err());
    let recipe = Recipe::new()
        .then(Step::new(QuarterTurn::Clockwise).masked(Mask::rectangle(0.0, 0.0, 2.0, 2.0)));
    assert!(grey.develop(&recipe).is_err());
}

#[test]
fn steps_run_in_order() {
    let source = picture(8, 8, |x, _| [(x * 30) as u8, 0, 0, 255]);
    let twice = developed(&source, Recipe::new().then(Invert).then(Invert));
    assert_eq!(rgba(&twice), rgba(&source));
}

#[test]
fn preview_uses_halved_proxy_with_scaled_steps() {
    let source = flat(64, 32, [100, 100, 100, 255]);
    let mut preview = Preview::new(source);
    let small = preview
        .render(&Recipe::new().then(GaussianBlur::new(8.0)), 0.2)
        .unwrap();
    assert_eq!(small.dimensions(), (16, 8));
    assert!(close(at(&small, 8, 4)[0], 100, 1));
}
