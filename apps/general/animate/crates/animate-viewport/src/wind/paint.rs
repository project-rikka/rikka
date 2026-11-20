use std::f32::consts::TAU;

use animate_stage::{Lens, Vane};
use animate_ui::{disc, stroke};
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

use crate::wind::grip::Grip;

const WIND: Color = Color::rgb(0.55, 0.92, 0.8);
const SHAFT: f32 = 2.4;
const HEAD: f32 = 14.0;
const HEAD_SPREAD: f32 = 0.45;
const HANDLE: f32 = 7.0;
const HANDLE_LIT: f32 = 1.3;
const OUTLINE: f32 = 1.5;
const RING: f32 = 0.6;
const RING_POINTS: usize = 48;
const RING_DOT: f32 = 2.0;
const STREAK_GAP: f32 = 11.0;
const STREAK_DOT: f32 = 1.6;
const STREAK_STEP: f32 = 4.0;
const STREAK_WAVES: f32 = 3.0;
const STREAK_SWAY: f32 = 6.0;
const CALM: f32 = 0.35;
const FULL_STRENGTH: f32 = 10.0;

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, vane: &Vane, frame: Rect, grip: &Grip) {
    let lens = Lens::new(vane.view_projection, frame);
    let strength = (vane.wind.speed / FULL_STRENGTH).clamp(CALM, 1.0);
    ctx.clipped(frame, |ctx| {
        ring(ctx, &lens, vane.anchor, WIND.scale_alpha(0.35));
        let (Some(tail), Some(tip)) = (lens.project(vane.tail()), lens.project(vane.tip())) else {
            return;
        };
        let tint = WIND.scale_alpha(0.45 + 0.5 * strength);
        if vane.wind.speed > 0.0 {
            streaks(
                ctx,
                tail,
                tip,
                vane.wind.turbulence,
                WIND.scale_alpha(0.5 * strength),
            );
        }
        stroke(ctx, tail, tip, SHAFT, Color::BLACK.scale_alpha(0.3));
        stroke(ctx, tail, tip, SHAFT * 0.7, tint);
        head(ctx, tail, tip, tint);
        let size = match grip.lit() {
            true => HANDLE * HANDLE_LIT,
            false => HANDLE,
        };
        let fill = match grip.lit() {
            true => WIND.scale_alpha(0.8),
            false => WIND.scale_alpha(0.35),
        };
        ctx.draw_quad(disc(tip, size + OUTLINE).fill(Color::BLACK.scale_alpha(0.35)));
        ctx.draw_quad(disc(tip, size).fill(fill).stroke(OUTLINE, WIND));
    });
}

fn ring(ctx: &mut PaintCtx<'_>, lens: &Lens, centre: Vec3, tint: Color) {
    for step in 0..RING_POINTS {
        let turn = TAU * step as f32 / RING_POINTS as f32;
        let point = centre + Vec3::new(turn.cos(), 0.0, turn.sin()) * RING;
        if let Some(at) = lens.project(point) {
            ctx.draw_circle(at, RING_DOT, tint);
        }
    }
}

fn head(ctx: &mut PaintCtx<'_>, tail: Point, tip: Point, tint: Color) {
    let length = tail.distance(tip);
    if length < 1.0 {
        return;
    }
    let back = (tail - tip) * (1.0 / length);
    for side in [-HEAD_SPREAD, HEAD_SPREAD] {
        let (sin, cos) = side.sin_cos();
        let barb = Point::new(back.x * cos - back.y * sin, back.x * sin + back.y * cos);
        stroke(ctx, tip, tip + barb * HEAD, SHAFT * 0.7, tint);
    }
}

fn streaks(ctx: &mut PaintCtx<'_>, tail: Point, tip: Point, turbulence: f32, tint: Color) {
    let length = tail.distance(tip);
    if length < 1.0 {
        return;
    }
    let along = (tip - tail) * (1.0 / length);
    let beside = Point::new(-along.y, along.x);
    let steps = (length / STREAK_STEP).floor() as usize;
    for side in [-1.0f32, 1.0] {
        for step in 0..=steps {
            let share = step as f32 / steps.max(1) as f32;
            let sway = (share * STREAK_WAVES * TAU).sin() * turbulence * STREAK_SWAY;
            let at = tail + (tip - tail) * share + beside * (side * STREAK_GAP + sway);
            ctx.draw_circle(at, STREAK_DOT, tint);
        }
    }
}
