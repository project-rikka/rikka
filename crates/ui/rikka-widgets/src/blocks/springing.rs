use rikka_ui::{Curve, PaintCtx};

const NUDGE: f32 = 0.5;

const HEIGHT: &str = "height";

pub(crate) fn spring_height(
    ctx: &mut PaintCtx<'_>,
    shown: &mut Option<f32>,
    target: f32,
    curve: Curve,
) -> f32 {
    let sprung = ctx.animate_curve(HEIGHT, target, curve);
    let height = ctx.snap(sprung.max(0.0));
    let moved = shown.replace(height) != Some(height);
    let arrived = sprung == target;
    if moved && (arrived || (height - ctx.bounds().height()).abs() >= NUDGE) {
        ctx.request_layout();
    }
    height
}
