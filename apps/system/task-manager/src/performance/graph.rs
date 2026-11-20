use rikka_ui::palette;
use std::rc::Rc;

use rikka_app::prelude::{PaintCtx, Quad, RoundedRect, Widget};
use rikka_types::{Color, Rect};

use crate::performance::history::SPAN;

const STEP: f32 = 2.0;
const LINE: f32 = 1.5;
const FILL: f32 = 0.28;

const RULE: f32 = 0.7;
const RULES: usize = 10;

pub(crate) fn trace(
    ctx: &mut PaintCtx<'_>,
    plot: Rect,
    samples: &[f32],
    scale: f32,
    colour: Color,
    filled: bool,
) {
    if samples.is_empty() || scale <= 0.0 {
        return;
    }
    let steps = (plot.size.width / STEP).ceil().max(1.0) as usize;
    let mut previous = height(plot, samples, scale, 0.0);
    for step in 0..=steps {
        let along = step as f32 / steps as f32;
        let y = height(plot, samples, scale, along);
        let x = plot.origin.x + along * plot.size.width;
        if filled {
            ctx.draw_quad(
                Quad::new(Rect::new(x, y, STEP, (plot.max_y() - y).max(0.0)))
                    .fill(colour.with_alpha(FILL)),
            );
        }
        let top = y.min(previous);
        let span = (y - previous).abs().max(LINE);
        ctx.draw_quad(Quad::new(Rect::new(x, top, STEP, span)).fill(colour));
        previous = y;
    }
}

fn height(plot: Rect, samples: &[f32], scale: f32, along: f32) -> f32 {
    let position = along * SPAN as f32 - (SPAN - samples.len()) as f32;
    if position < 0.0 {
        return plot.max_y();
    }
    let index = (position.floor() as usize).min(samples.len() - 1);
    let next = (index + 1).min(samples.len() - 1);
    let blend = position - position.floor();
    let value = samples[index] + (samples[next] - samples[index]) * blend;
    let fraction = (value / scale).clamp(0.0, 1.0);
    plot.max_y() - fraction * plot.size.height
}

pub(crate) struct Graph {
    pub(crate) samples: Rc<Vec<f32>>,
    pub(crate) second: Option<Rc<Vec<f32>>>,
    pub(crate) scale: f32,
    pub(crate) colour: Color,
    pub(crate) ruled: bool,
}

impl Widget for Graph {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = palette();
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface)
                .stroke(1.0, palette.outline),
        );
        if self.ruled {
            rule(ctx, bounds, palette.outline.scale_alpha(RULE));
        }
        let plot = bounds.inflate(-1.0);
        ctx.clipped(RoundedRect::new(plot, 0.0), |ctx| {
            if let Some(second) = &self.second {
                let under = self.colour.mix(palette.text, 0.35);
                trace(ctx, plot, second, self.scale, under, false);
            }
            trace(ctx, plot, &self.samples, self.scale, self.colour, true);
        });
    }
}

fn rule(ctx: &mut PaintCtx<'_>, bounds: Rect, colour: Color) {
    let plot = bounds.inflate(-1.0);
    for line in 1..RULES {
        let along = line as f32 / RULES as f32;
        ctx.draw_quad(
            Quad::new(Rect::new(
                plot.origin.x,
                (plot.origin.y + plot.size.height * along).round(),
                plot.size.width,
                1.0,
            ))
            .fill(colour),
        );
        ctx.draw_quad(
            Quad::new(Rect::new(
                (plot.origin.x + plot.size.width * along).round(),
                plot.origin.y,
                1.0,
                plot.size.height,
            ))
            .fill(colour),
        );
    }
}
