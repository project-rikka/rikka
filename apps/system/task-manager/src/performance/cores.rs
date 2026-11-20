use rikka_ui::palette;
use std::rc::Rc;

use rikka_app::prelude::{PaintCtx, Quad, RoundedRect, Widget};
use rikka_types::{Color, Rect};

use crate::performance::graph;

const GAP: f32 = 6.0;
const ACROSS: usize = 8;

pub(crate) struct Cores {
    pub(crate) series: Vec<Rc<Vec<f32>>>,
    pub(crate) colour: Color,
}

impl Widget for Cores {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        if self.series.is_empty() {
            return;
        }
        let bounds = ctx.bounds();
        let palette = palette();
        let across = ACROSS.min(self.series.len());
        let down = self.series.len().div_ceil(across);
        let width = (bounds.size.width - GAP * (across - 1) as f32) / across as f32;
        let height = (bounds.size.height - GAP * (down.saturating_sub(1)) as f32) / down as f32;

        for (index, samples) in self.series.iter().enumerate() {
            let cell = Rect::new(
                bounds.origin.x + (index % across) as f32 * (width + GAP),
                bounds.origin.y + (index / across) as f32 * (height + GAP),
                width.max(0.0),
                height.max(0.0),
            );
            ctx.draw_quad(
                Quad::new(cell)
                    .fill(palette.surface)
                    .stroke(1.0, palette.outline),
            );
            let plot = cell.inflate(-1.0);
            ctx.clipped(RoundedRect::new(plot, 0.0), |ctx| {
                graph::trace(ctx, plot, samples, 1.0, self.colour, true);
            });
        }
    }
}
