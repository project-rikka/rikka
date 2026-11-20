use rikka_renderer_3d::glam::{Vec2, Vec3};
use rikka_renderer_3d::{BuiltinEffect, EffectInstance};

use crate::look::canvas::{CENTER, Canvas, Paint, SIZE, Shape, chain};

fn at_1080p(pixels: f32) -> f32 {
    pixels * 1080.0 / SIZE as f32
}

#[test]
fn streaks_follow_their_rays() {
    let mut canvas = Canvas::new();
    let streaks = canvas.effect(BuiltinEffect::Streaks);
    let source = canvas.paint(Paint::new(Shape::Square, Vec3::splat(400.0)).extent(2.0));
    let instance = EffectInstance::new(streaks)
        .set("amount", 1.0)
        .set("count", rikka_renderer_3d::Value::Int(4))
        .set("angle", 0.0)
        .set("length", at_1080p(60.0));
    let shot = canvas.shoot(chain([source.into(), instance.into()]));
    for (x, y) in [(20, 0), (-20, 0), (0, 20), (0, -20)] {
        let along = shot.linear(CENTER + x, CENTER + y).y;
        let across = shot
            .linear(CENTER + (x + y) * 7 / 10, CENTER + (y - x) * 7 / 10)
            .y;
        assert!(
            along > 0.005 && along > 4.0 * across,
            "ray at {x},{y} is {along}, between rays {across}"
        );
    }
}

#[test]
fn fog_glow_falls_off_slowly_and_reaches_far() {
    let mut canvas = Canvas::new();
    let fog = canvas.effect(BuiltinEffect::FogGlow);
    let source = canvas.paint(Paint::new(Shape::Square, Vec3::splat(400.0)).extent(2.0));
    let instance = EffectInstance::new(fog).set("amount", 1.0);
    let shot = canvas.shoot(chain([source.into(), instance.into()]));
    let glow: Vec<f32> = [6, 15, 35, 70]
        .iter()
        .map(|&distance| shot.linear(CENTER + distance, CENTER).y)
        .collect();
    assert!(
        glow.windows(2).all(|pair| pair[0] > pair[1]),
        "glow does not fall off: {glow:?}"
    );
    assert!(glow[3] > 0.002, "glow does not reach 70 px: {glow:?}");
}

#[test]
fn lens_flare_ghosts_mirror_through_the_centre() {
    let mut canvas = Canvas::new();
    let flare = canvas.effect(BuiltinEffect::LensFlare);
    let offset = Vec2::splat(-64.0);
    let source = canvas.paint(
        Paint::new(Shape::Square, Vec3::splat(200.0))
            .extent(6.0)
            .offset(offset),
    );
    let plain = canvas.shoot(chain([source.clone().into()]));
    let instance = EffectInstance::new(flare)
        .set("amount", 1.0)
        .set("halo.amount", 0.0)
        .set("starburst.amount", 0.0);
    let shot = canvas.shoot(chain([source.into(), instance.into()]));
    let mirrored = (CENTER - offset.x as i32 - 1, CENTER - offset.y as i32 - 1);
    let ghost = shot.linear(mirrored.0, mirrored.1).y - plain.linear(mirrored.0, mirrored.1).y;
    let elsewhere = shot.linear(mirrored.0, CENTER + offset.y as i32).y;
    assert!(
        ghost > 0.005 && ghost > 4.0 * elsewhere,
        "ghost {ghost} at the mirrored point, {elsewhere} elsewhere"
    );
}

#[test]
fn halation_tints_the_surround_of_highlights_red() {
    let mut canvas = Canvas::new();
    let halation = canvas.effect(BuiltinEffect::Halation);
    let source = canvas.paint(Paint::new(Shape::Disc, Vec3::splat(8.0)).extent(2.0));
    let instance = EffectInstance::new(halation)
        .set("amount", 0.5)
        .set("radius", at_1080p(5.0));
    let shot = canvas.shoot(chain([source.into(), instance.into()]));
    for distance in [5, 8] {
        let surround = shot.linear(CENTER + distance, CENTER);
        assert!(
            surround.x > 0.005
                && surround.x < 0.9
                && surround.x > 3.0 * surround.y
                && surround.x > 3.0 * surround.z,
            "surround at {distance} px is {surround:?}"
        );
    }
}
