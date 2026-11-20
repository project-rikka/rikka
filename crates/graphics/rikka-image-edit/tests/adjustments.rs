use std::fmt::Write as _;

use crate::support::{at, close, developed, flat, picture};
use rikka_image_edit::{
    AutoColor, AutoContrast, AutoLevels, Channel, ColorLookup, Curves, Equalize, GradientMap,
    GradientStop, HueSaturation, Invert, Level, Levels, Recipe, Shift, WhiteBalance,
};
use rikka_types::Color;

#[test]
fn invert_keeps_alpha() {
    let inverted = developed(&flat(3, 3, [10, 100, 250, 77]), Recipe::new().then(Invert));
    assert_eq!(at(&inverted, 1, 1), [245, 155, 5, 77]);
}

#[test]
fn levels_match_photoshop() {
    let levels = Levels::new().channel(Channel::Composite, Level::new().input(0.2, 0.6));
    let leveled = developed(&flat(2, 2, [102, 51, 200, 255]), Recipe::new().then(levels));
    let [r, g, b, _] = at(&leveled, 0, 0);
    assert!(close(r, 128, 1), "{r}");
    assert_eq!(g, 0);
    assert_eq!(b, 255);
}

#[test]
fn identity_curve_is_noop() {
    let source = picture(16, 1, |x, _| [(x * 16) as u8, 128, 64, 255]);
    let straight = developed(
        &source,
        Recipe::new().then(Curves::new().channel(Channel::Composite, [(0.0, 0.0), (1.0, 1.0)])),
    );
    assert_eq!(
        crate::support::rgba(&straight),
        crate::support::rgba(&source)
    );
    let lifted = developed(
        &source,
        Recipe::new()
            .then(Curves::new().channel(Channel::Red, [(0.0, 0.0), (0.5, 0.75), (1.0, 1.0)])),
    );
    let [r, g, _, _] = at(&lifted, 8, 0);
    assert!(close(r, 191, 2), "{r}");
    assert_eq!(g, 128, "only red curve applies");
}

#[test]
fn hue_shift_rotates_red_to_green() {
    let red = flat(2, 2, [255, 0, 0, 255]);
    let [r, g, b, _] = at(
        &developed(
            &red,
            Recipe::new().then(HueSaturation::new().master(Shift::new(120.0, 0.0, 0.0))),
        ),
        0,
        0,
    );
    assert!(r < 5 && g > 250 && b < 5, "{r},{g},{b}");
}

#[test]
fn gradient_map_follows_luminance() {
    let ramp = picture(3, 1, |x, _| {
        [(x * 127) as u8, (x * 127) as u8, (x * 127) as u8, 255]
    });
    let map = GradientMap::new([
        GradientStop::new(0.0, Color::rgb(1.0, 0.0, 0.0)),
        GradientStop::new(1.0, Color::rgb(0.0, 0.0, 1.0)),
    ]);
    let painted = developed(&ramp, Recipe::new().then(map));
    assert_eq!(at(&painted, 0, 0), [255, 0, 0, 255]);
    let [r, _, b, _] = at(&painted, 2, 0);
    assert!(r < 10 && b > 245);
}

#[test]
fn identity_cube_lut_is_noop_and_garbage_rejected() {
    let mut text = String::from("LUT_3D_SIZE 2\n");
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                writeln!(text, "{r} {g} {b}").expect("write to String");
            }
        }
    }
    let lookup = ColorLookup::parse(&text).expect("valid cube file");
    let source = picture(4, 4, |x, y| [(x * 60) as u8, (y * 60) as u8, 90, 255]);
    let looked = developed(&source, Recipe::new().then(lookup));
    for (a, b) in crate::support::rgba(&looked)
        .iter()
        .zip(crate::support::rgba(&source))
    {
        assert!(close(*a, b, 1));
    }

    assert!(ColorLookup::parse("hello").is_err());
}

#[test]
fn white_balance_daylight_noop_tungsten_cools() {
    let warm = flat(2, 2, [200, 150, 100, 255]);
    let same = at(
        &developed(&warm, Recipe::new().then(WhiteBalance::default())),
        0,
        0,
    );
    assert!(close(same[0], 200, 1) && close(same[2], 100, 1), "{same:?}");
    let cooled = at(
        &developed(&warm, Recipe::new().then(WhiteBalance::new(3200.0))),
        0,
        0,
    );
    assert!(cooled[2] > 120 && cooled[0] < 200, "{cooled:?}");
}

#[test]
fn auto_corrections_stretch_contrast() {
    let dull = picture(64, 1, |x, _| {
        let v = 80 + x as u8;
        [v, v.saturating_sub(10), v.saturating_sub(20), 255]
    });
    for recipe in [
        Recipe::new().then(AutoLevels::new()),
        Recipe::new().then(AutoContrast::new()),
        Recipe::new().then(AutoColor::new()),
        Recipe::new().then(Equalize),
    ] {
        let stretched = developed(&dull, recipe.clone());
        let darkest = at(&stretched, 0, 0);
        let lightest = at(&stretched, 63, 0);
        let low = darkest[..3].iter().copied().min().unwrap_or(255);
        let high = lightest[..3].iter().copied().max().unwrap_or(0);
        assert!(
            low < 30 && high > 225,
            "{recipe:?}: {darkest:?} {lightest:?}"
        );
    }
}
