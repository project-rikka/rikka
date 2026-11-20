use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{BuiltinEffect, EffectInstance, Value};

use crate::look::canvas::{CENTER, Canvas, Paint, Shape, chain};

const FRAME: f32 = 1.0 / 60.0;

fn brightness_over_frames(
    canvas: &mut Canvas,
    instance: &EffectInstance,
    inputs: &[f32],
) -> Vec<f32> {
    let mut viewport = canvas.viewport();
    inputs
        .iter()
        .enumerate()
        .map(|(frame, &value)| {
            let painted = canvas.paint(Paint::new(Shape::Flat, Vec3::splat(value)));
            let composite = chain([painted.into(), instance.clone().into()]);
            canvas
                .shoot_on(&mut viewport, composite, frame as f32 * FRAME)
                .linear(CENTER, CENTER)
                .y
        })
        .collect()
}

#[test]
fn trails_decay_geometrically() {
    let mut canvas = Canvas::new();
    let trails = canvas.effect(BuiltinEffect::Trails);
    for mode in ["lighten", "blend"] {
        let instance = EffectInstance::new(trails)
            .set("persistence", 0.5)
            .set("mode", Value::Enum(mode.into()));
        let seen = brightness_over_frames(&mut canvas, &instance, &[0.8, 0.0, 0.0, 0.0, 0.0]);
        assert!(
            (seen[0] - 0.8).abs() < 0.01,
            "{mode}: first frame {}",
            seen[0]
        );
        for (frame, pair) in seen.windows(2).skip(1).enumerate() {
            let ratio = pair[1] / pair[0];
            assert!(
                (ratio - 0.5).abs() < 0.04,
                "{mode}: frame {} kept {ratio} of the last: {seen:?}",
                frame + 2
            );
        }
        assert!((seen[1] - 0.4).abs() < 0.01, "{mode}: {seen:?}");
    }
}

#[test]
fn frame_blend_mixes_only_the_previous_frame() {
    let mut canvas = Canvas::new();
    let blend = canvas.effect(BuiltinEffect::FrameBlend);
    let instance = EffectInstance::new(blend).set("shutter", 1.0);
    let seen = brightness_over_frames(&mut canvas, &instance, &[0.8, 0.0, 0.0]);
    assert!((seen[0] - 0.8).abs() < 0.01, "{seen:?}");
    assert!((seen[1] - 0.4).abs() < 0.01, "{seen:?}");
    assert!(seen[2] < 0.002, "an older frame lingers: {seen:?}");
}

#[test]
fn datamosh_keeps_dragging_earlier_frames() {
    let mut canvas = Canvas::new();
    let mosh = canvas.effect(BuiltinEffect::Datamosh);
    let instance = EffectInstance::new(mosh)
        .set("amount", 1.0)
        .set("refresh", 0.0);
    let mut viewport = canvas.viewport();
    let card = canvas.paint(Paint::new(Shape::Card, Vec3::ONE));
    let grey = canvas.paint(Paint::new(Shape::Flat, Vec3::splat(0.5)));
    let first = canvas.shoot_on(
        &mut viewport,
        chain([card.into(), instance.clone().into()]),
        0.0,
    );
    let second = canvas.shoot_on(&mut viewport, chain([grey.into(), instance.into()]), FRAME);
    let difference = first.largest_difference(&second);
    assert!(
        difference <= 2,
        "the new frame broke through by {difference}"
    );
}
