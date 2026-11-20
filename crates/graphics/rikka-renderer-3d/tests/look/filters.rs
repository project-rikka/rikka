use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{BuiltinEffect, EffectInstance, Value};

use crate::look::canvas::{CENTER, Canvas, Paint, SIZE, Shape, chain, sized};

const REFERENCE_HEIGHT: f32 = 1080.0;

fn at_1080p(pixels: f32) -> f32 {
    pixels * REFERENCE_HEIGHT / SIZE as f32
}

#[test]
fn filters_are_neutral_at_their_neutral_settings() {
    let mut canvas = Canvas::new();
    let card = canvas.paint(Paint::new(Shape::Card, Vec3::ONE));
    let plain = canvas.shoot(chain([card.clone().into()]));
    let cases: [(BuiltinEffect, &[(&str, f32)]); 21] = [
        (BuiltinEffect::GaussianBlur, &[("radius", 0.0)]),
        (BuiltinEffect::DirectionalBlur, &[("length", 0.0)]),
        (BuiltinEffect::BokehBlur, &[("radius", 0.0)]),
        (
            BuiltinEffect::DilateErode,
            &[("distance", 0.0), ("feather", 0.0)],
        ),
        (BuiltinEffect::Median, &[("amount", 0.0)]),
        (BuiltinEffect::UnsharpMask, &[("amount", 0.0)]),
        (BuiltinEffect::Bilateral, &[("amount", 0.0)]),
        (BuiltinEffect::EdgeDetect, &[("amount", 0.0)]),
        (BuiltinEffect::Streaks, &[("amount", 0.0)]),
        (BuiltinEffect::FogGlow, &[("amount", 0.0)]),
        (BuiltinEffect::LensFlare, &[("amount", 0.0)]),
        (BuiltinEffect::Halation, &[("amount", 0.0)]),
        (
            BuiltinEffect::GateWeave,
            &[("weave", 0.0), ("rotation", 0.0), ("flicker", 0.0)],
        ),
        (BuiltinEffect::Cdl, &[]),
        (BuiltinEffect::WhiteBalance, &[]),
        (BuiltinEffect::Trails, &[("persistence", 0.0)]),
        (BuiltinEffect::FrameBlend, &[("shutter", 0.0)]),
        (BuiltinEffect::Datamosh, &[("amount", 0.0)]),
        (
            BuiltinEffect::LensDistortion,
            &[("amount", 0.0), ("cubic", 0.0), ("dispersion", 0.0)],
        ),
        (BuiltinEffect::FilmGrain, &[("amount", 0.0)]),
        (BuiltinEffect::Vignette, &[("strength", 0.0)]),
    ];
    for (builtin, settings) in cases {
        let id = canvas.effect(builtin);
        let instance = settings
            .iter()
            .fold(EffectInstance::new(id), |instance, &(name, value)| {
                instance.set(name, value)
            });
        let shown = canvas.shoot(chain([card.clone().into(), instance.into()]));
        let difference = plain.largest_difference(&shown);
        assert!(
            difference <= 1,
            "{builtin:?} changes a neutral image by {difference}"
        );
    }
}

struct Moments {
    energy: f32,
    center: (f32, f32),
    sigma: f32,
}

fn moments(shot: &crate::look::canvas::Shot, reach: i32) -> Moments {
    let mut energy = 0.0;
    let mut first = (0.0, 0.0);
    let window = (CENTER - reach).max(0)..(CENTER + reach + 1).min(SIZE as i32);
    for y in window.clone() {
        for x in window.clone() {
            let value = shot.linear(x, y).y;
            energy += value;
            first.0 += value * x as f32;
            first.1 += value * y as f32;
        }
    }
    let center = (first.0 / energy, first.1 / energy);
    let mut second = 0.0;
    for y in window.clone() {
        for x in window.clone() {
            let value = shot.linear(x, y).y;
            let (dx, dy) = (x as f32 - center.0, y as f32 - center.1);
            second += value * (dx * dx + dy * dy);
        }
    }
    Moments {
        energy,
        center,
        sigma: (second / energy / 2.0).sqrt(),
    }
}

#[test]
fn gaussian_blur_spreads_an_impulse_with_its_sigma_and_keeps_its_energy() {
    let mut canvas = Canvas::new();
    let blur = canvas.effect(BuiltinEffect::GaussianBlur);
    for sigma in [1.5_f32, 6.0, 20.0] {
        let energy = 0.6 * std::f32::consts::TAU * sigma * sigma;
        let impulse = canvas.paint(Paint::new(Shape::Impulse, Vec3::splat(energy)));
        let blurred = EffectInstance::new(blur).set("radius", at_1080p(sigma));
        let shot = canvas.shoot(chain([impulse.into(), blurred.into()]));
        let found = moments(&shot, (sigma * 4.5).ceil() as i32 + 2);
        assert!(
            (found.energy / energy - 1.0).abs() < 0.06,
            "σ {sigma}: energy {} instead of {energy}",
            found.energy
        );
        assert!(
            (found.center.0 - CENTER as f32).abs() < 0.35
                && (found.center.1 - CENTER as f32).abs() < 0.35,
            "σ {sigma}: centred at {:?}",
            found.center
        );
        assert!(
            (found.sigma / sigma - 1.0).abs() < 0.08,
            "σ {sigma}: measured σ {}",
            found.sigma
        );
    }
}

#[test]
fn variable_blur_leaves_unmasked_pixels_untouched() {
    let mut canvas = Canvas::new();
    let blur = canvas.effect(BuiltinEffect::GaussianBlur);
    let bokeh = canvas.effect(BuiltinEffect::BokehBlur);
    let card = canvas.paint(Paint::new(Shape::Card, Vec3::ONE));
    let mask = canvas.paint(Paint::new(Shape::Step, Vec3::ONE));
    let plain = canvas.shoot(chain([card.clone().into()]));
    for (name, instance) in [
        ("gaussian", EffectInstance::new(blur).set("radius", 30.0)),
        ("bokeh", EffectInstance::new(bokeh).set("radius", 40.0)),
    ] {
        let shot = canvas.shoot(sized(card.clone(), mask.clone(), instance, "size"));
        let mut changed = 0.0;
        for y in 0..SIZE as i32 {
            for x in 0..CENTER {
                assert_eq!(
                    shot.rgb(x, y),
                    plain.rgb(x, y),
                    "{name}: masked-out pixel {x},{y} changed"
                );
            }
            for x in CENTER + 16..SIZE as i32 {
                let (a, b) = (shot.rgb(x, y), plain.rgb(x, y));
                changed += f64::from(a[0].abs_diff(b[0]));
            }
        }
        let mean = changed / f64::from((SIZE - CENTER as u32 - 16) * SIZE);
        assert!(
            mean > 3.0,
            "{name}: masked-in pixels change by only {mean:.2}"
        );
    }
}

fn edge_radius(shot: &crate::look::canvas::Shot, angle: f32, half: f32) -> f32 {
    let (sin, cos) = angle.sin_cos();
    let mut radius = 0.0;
    while radius < 60.0 {
        let x = (CENTER as f32 + 0.5 + cos * radius).floor() as i32;
        let y = (CENTER as f32 + 0.5 + sin * radius).floor() as i32;
        if shot.linear(x, y).y < half {
            return radius;
        }
        radius += 0.25;
    }
    radius
}

#[test]
fn bokeh_takes_the_aperture_shape() {
    let mut canvas = Canvas::new();
    let bokeh = canvas.effect(BuiltinEffect::BokehBlur);
    let disc = canvas.paint(Paint::new(Shape::Disc, Vec3::splat(50.0)).extent(3.0));
    for blades in [0, 5, 6] {
        let instance = EffectInstance::new(bokeh)
            .set("radius", at_1080p(30.0))
            .set("blades", Value::Int(blades))
            .set("curvature", 0.0)
            .set("highlights.boost", 0.0);
        let shot = canvas.shoot(chain([disc.clone().into(), instance.into()]));
        let half = 0.5 * shot.linear(CENTER, CENTER).y;
        let radii: Vec<f32> = (0..360)
            .step_by(2)
            .map(|degrees| edge_radius(&shot, (degrees as f32).to_radians(), half))
            .collect();
        let widest = radii.iter().copied().fold(f32::MIN, f32::max);
        let narrowest = radii.iter().copied().fold(f32::MAX, f32::min);
        match blades {
            0 => assert!(
                widest - narrowest < 2.0,
                "round bokeh spans {narrowest}..{widest}"
            ),
            blades => {
                let turn = (360 / blades as usize) / 2;
                for (index, radius) in radii.iter().enumerate() {
                    let turned = radii[(index + turn) % radii.len()];
                    assert!(
                        (radius - turned).abs() <= 2.0,
                        "{blades} blades: {radius} at {}° but {turned} a blade further",
                        index * 2
                    );
                }
                assert!(
                    widest - narrowest > 2.0,
                    "{blades} blades look round: {narrowest}..{widest}"
                );
            }
        }
    }
}

#[test]
fn directional_blur_smears_along_its_angle_only() {
    let mut canvas = Canvas::new();
    let smear = canvas.effect(BuiltinEffect::DirectionalBlur);
    let impulse = canvas.paint(Paint::new(Shape::Impulse, Vec3::splat(20.0)));
    let instance = EffectInstance::new(smear)
        .set("angle", 0.0)
        .set("length", at_1080p(40.0));
    let shot = canvas.shoot(chain([impulse.into(), instance.into()]));
    for offset in [-15, -5, 5, 15] {
        let lit = shot.linear(CENTER + offset, CENTER).y;
        assert!(lit > 0.2, "{offset} px along the smear is only {lit}");
    }
    for (x, y) in [(0, 6), (0, -6), (26, 0), (-26, 0)] {
        let dark = shot.linear(CENTER + x, CENTER + y).y;
        assert!(dark < 0.01, "{x},{y} off the smear is {dark}");
    }
}

fn lit_run(shot: &crate::look::canvas::Shot, vertical: bool) -> i32 {
    (0..SIZE as i32)
        .filter(|&at| {
            let (x, y) = match vertical {
                true => (CENTER, at),
                false => (at, CENTER),
            };
            shot.linear(x, y).y > 0.5
        })
        .count() as i32
}

#[test]
fn dilate_grows_and_erode_shrinks_by_the_distance() {
    let mut canvas = Canvas::new();
    let morph = canvas.effect(BuiltinEffect::DilateErode);
    for (side, distance) in [(8.0, 10.0), (40.0, -10.0)] {
        let square = canvas.paint(Paint::new(Shape::Square, Vec3::ONE).extent(side));
        let instance = EffectInstance::new(morph)
            .set("distance", at_1080p(distance))
            .set("feather", 0.0);
        let shot = canvas.shoot(chain([square.into(), instance.into()]));
        let expected = side as i32 + 2 * distance as i32;
        for vertical in [false, true] {
            let run = lit_run(&shot, vertical);
            assert!(
                (run - expected).abs() <= 2,
                "{side} px square changed by {distance}: {run} px instead of {expected}"
            );
        }
    }
}

#[test]
fn median_removes_a_speckle() {
    let mut canvas = Canvas::new();
    let median = canvas.effect(BuiltinEffect::Median);
    let speckle = canvas.paint(Paint::new(Shape::Impulse, Vec3::ONE).background(Vec3::splat(0.3)));
    for threshold in [0.0, 0.5] {
        let instance = EffectInstance::new(median).set("threshold", threshold);
        let shot = canvas.shoot(chain([speckle.clone().into(), instance.into()]));
        let left = shot.linear(CENTER, CENTER).y;
        assert!((left - 0.3).abs() < 0.01, "speckle left at {left}");
    }
}

#[test]
fn edge_detection_fires_on_a_step_only() {
    let mut canvas = Canvas::new();
    let edges = canvas.effect(BuiltinEffect::EdgeDetect);
    let step = canvas.paint(Paint::new(Shape::Step, Vec3::splat(0.6)).background(Vec3::splat(0.2)));
    for kernel in ["sobel", "prewitt", "kirsch", "laplacian"] {
        let instance = EffectInstance::new(edges).set("kernel", Value::Enum(kernel.into()));
        let shot = canvas.shoot(chain([step.clone().into(), instance.into()]));
        let edge = shot.linear(CENTER - 1, CENTER).y;
        assert!((edge - 0.4).abs() < 0.03, "{kernel}: edge strength {edge}");
        for x in [8, CENTER - 4, CENTER + 4, SIZE as i32 - 8] {
            let flat = shot.linear(x, CENTER).y;
            assert!(flat < 0.001, "{kernel}: flat pixel {x} fired with {flat}");
        }
    }
}

fn deviation(shot: &crate::look::canvas::Shot, columns: std::ops::Range<i32>) -> f32 {
    let values: Vec<f32> = columns
        .flat_map(|x| (100..140).map(move |y| (x, y)))
        .map(|(x, y)| shot.linear(x, y).y)
        .collect();
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32).sqrt()
}

#[test]
fn bilateral_smooths_noise_and_keeps_edges() {
    let mut canvas = Canvas::new();
    let bilateral = canvas.effect(BuiltinEffect::Bilateral);
    let noisy =
        canvas.paint(Paint::new(Shape::NoisyStep, Vec3::splat(0.6)).background(Vec3::splat(0.2)));
    let plain = canvas.shoot(chain([noisy.clone().into()]));
    let instance = EffectInstance::new(bilateral)
        .set("radius", at_1080p(3.0))
        .set("tolerance", 0.1);
    let shot = canvas.shoot(chain([noisy.into(), instance.into()]));
    let before = deviation(&plain, 20..60);
    let after = deviation(&shot, 20..60);
    assert!(after < 0.5 * before, "noise {before} only fell to {after}");
    let step = |shot: &crate::look::canvas::Shot| {
        (100..140)
            .map(|y| shot.linear(CENTER + 1, y).y - shot.linear(CENTER - 2, y).y)
            .sum::<f32>()
            / 40.0
    };
    assert!(
        step(&shot) > 0.85 * step(&plain),
        "edge fell from {} to {}",
        step(&plain),
        step(&shot)
    );
}
