use std::rc::Rc;
use std::time::{Duration, SystemTime};

use rikka_app::prelude::{PaintCtx, Quad, Rect, RoundedRect, Widget, palette};
use rikka_battery::{Charge, Sample};

const RULES: usize = 4;
const RULE_ALPHA: f32 = 0.6;
const FILL_ALPHA: f32 = 0.85;
const GAP: f32 = 1.0;
const LONGEST_STEP: Duration = Duration::from_secs(30 * 60);

pub(crate) struct Chart {
    pub(crate) samples: Rc<Vec<Sample>>,
    pub(crate) span: Duration,
    pub(crate) until: SystemTime,
}

impl Widget for Chart {
    type State = ();

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = palette();
        ctx.draw_quad(
            Quad::new(bounds)
                .fill(palette.surface_variant)
                .stroke(1.0, palette.outline),
        );
        let plot = bounds.inflate(-1.0);
        for rule in 1..RULES {
            let y = (plot.origin.y + plot.size.height * rule as f32 / RULES as f32).round();
            ctx.draw_quad(
                Quad::new(Rect::new(plot.origin.x, y, plot.size.width, 1.0))
                    .fill(palette.outline.scale_alpha(RULE_ALPHA)),
            );
        }
        let Some(start) = self.until.checked_sub(self.span) else {
            return;
        };
        let along = |at: SystemTime| {
            let into = at.duration_since(start).unwrap_or_default().as_secs_f32();
            plot.origin.x + (into / self.span.as_secs_f32()).clamp(0.0, 1.0) * plot.size.width
        };
        ctx.clipped(RoundedRect::new(plot, 0.0), |ctx| {
            for (index, sample) in self.samples.iter().enumerate() {
                if sample.at < start {
                    continue;
                }
                let ends = self
                    .samples
                    .get(index + 1)
                    .map(|next| next.at)
                    .unwrap_or(self.until)
                    .min(sample.at + LONGEST_STEP);
                let left = along(sample.at);
                let width = (along(ends) - left - GAP).max(1.0);
                let height = plot.size.height * sample.charge;
                let colour = match sample.state {
                    Charge::Charging | Charge::Full => palette.success,
                    _ if sample.charge <= 0.2 => palette.warning,
                    _ => palette.accent,
                };
                ctx.draw_quad(
                    Quad::new(Rect::new(left, plot.max_y() - height, width, height))
                        .fill(colour.with_alpha(FILL_ALPHA)),
                );
            }
        });
    }
}
