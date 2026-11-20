use cut_project::{AnimationPreset, AnimationPresets, Clip};
use rikka_app::prelude::*;

use crate::timeline::scale::Scale;

const STRIP: f32 = 2.0;
const MAX_STRIPS: usize = 256;
const FILL: f32 = 0.22;
const EDGE_LINE: f32 = 1.5;
const EDGE_INK: f32 = 0.7;
const REACH: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WedgeSide {
    In,
    Out,
}

impl WedgeSide {
    fn animation_preset(self, animation_presets: &AnimationPresets) -> Option<AnimationPreset> {
        match self {
            WedgeSide::In => animation_presets.in_preset,
            WedgeSide::Out => animation_presets.out_preset,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Wedge {
    pub(crate) side: WedgeSide,
    pub(crate) animation_preset: AnimationPreset,
    pub(crate) from: i64,
    pub(crate) to: i64,
}

impl Wedge {
    pub(crate) fn inner(&self) -> i64 {
        match self.side {
            WedgeSide::In => self.to,
            WedgeSide::Out => self.from,
        }
    }

    fn rest(&self, progress: f64) -> f32 {
        let eased = self.animation_preset.easing.at(progress).clamp(0.0, 1.0) as f32;
        match self.side {
            WedgeSide::In => eased,
            WedgeSide::Out => 1.0 - eased,
        }
    }
}

pub(crate) fn wedges(start: i64, length: i64, animation_presets: &AnimationPresets) -> Vec<Wedge> {
    let (enter, exit) = animation_presets.lengths(length);
    [
        (WedgeSide::In, enter, start, start + enter),
        (WedgeSide::Out, exit, start + length - exit, start + length),
    ]
    .into_iter()
    .filter_map(|(side, span, from, to)| {
        let animation_preset = side.animation_preset(animation_presets)?;
        (span > 0).then_some(Wedge {
            side,
            animation_preset,
            from,
            to,
        })
    })
    .collect()
}

pub(crate) fn edge_at(clip: &Clip, scale: &Scale, x: f32) -> Option<WedgeSide> {
    wedges(clip.start(), clip.length(), clip.animation_presets())
        .into_iter()
        .filter(|wedge| (scale.x_of(wedge.inner() as f64) - x).abs() <= REACH)
        .min_by(|left, right| {
            let gap = |wedge: &Wedge| (scale.x_of(wedge.inner() as f64) - x).abs();
            gap(left).total_cmp(&gap(right))
        })
        .map(|wedge| wedge.side)
}

pub(crate) fn resized(
    animation_presets: &AnimationPresets,
    side: WedgeSide,
    length: i64,
    requested: i64,
) -> AnimationPresets {
    let (enter, exit) = animation_presets.lengths(length);
    let other = match side {
        WedgeSide::In => exit,
        WedgeSide::Out => enter,
    };
    let span = requested.clamp(1, (length - other).max(1));
    let mut resized = *animation_presets;
    let animation_preset = match side {
        WedgeSide::In => &mut resized.in_preset,
        WedgeSide::Out => &mut resized.out_preset,
    };
    if let Some(animation_preset) = animation_preset {
        animation_preset.length = span;
    }
    resized
}

pub(crate) fn paint(
    ctx: &mut PaintCtx<'_>,
    body: Rect,
    left: f32,
    scale: &Scale,
    wedge: &Wedge,
    ink: Color,
) {
    let from = left + scale.x_of(wedge.from as f64);
    let to = left + scale.x_of(wedge.to as f64);
    let width = to - from;
    if width < 1.0 {
        return;
    }
    let strips = ((width / STRIP).ceil() as usize).clamp(1, MAX_STRIPS);
    let each = width / strips as f32;
    let fill = ink.scale_alpha(FILL);
    ctx.clipped(body, |ctx| {
        for strip in 0..strips {
            let progress = (strip as f64 + 0.5) / strips as f64;
            let height = body.height() * wedge.rest(progress);
            if height <= 0.0 {
                continue;
            }
            ctx.draw_quad(
                Quad::new(Rect::new(
                    from + strip as f32 * each,
                    body.max_y() - height,
                    each + 0.5,
                    height,
                ))
                .fill(fill),
            );
        }
        let edge = left + scale.x_of(wedge.inner() as f64);
        ctx.draw_quad(
            Quad::new(Rect::new(
                edge - EDGE_LINE / 2.0,
                body.y(),
                EDGE_LINE,
                body.height(),
            ))
            .fill(ink.scale_alpha(EDGE_INK)),
        );
    });
}
