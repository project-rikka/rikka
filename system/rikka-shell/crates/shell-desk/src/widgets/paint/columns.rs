use rikka_types::{Color, Rect};
use rikka_ui::{Layout, PaintCtx, Quad, Style, Widget};

use crate::widgets::paint::track::track;

const GAP_SHARE: f32 = 0.3;
const MAX_GAP: f32 = 3.0;
const LEAST: f32 = 0.04;

#[derive(Widget)]
pub(crate) struct Columns {
    #[debug]
    values: Vec<f32>,
    color: Color,
    #[style]
    style: Style,
}

#[must_use]
pub(crate) fn columns(values: impl IntoIterator<Item = f32>, color: Color) -> Columns {
    Columns {
        values: values
            .into_iter()
            .map(|value| value.clamp(0.0, 1.0))
            .collect(),
        color,
        style: Style::default(),
    }
}

impl Widget for Columns {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let count = self.values.len();
        if count == 0 || bounds.width() <= 0.0 {
            return;
        }
        let pitch = bounds.width() / count as f32;
        let gap = (pitch * GAP_SHARE).min(MAX_GAP);
        let width = pitch - gap;
        let round = (width / 2.0).min(2.0);
        let track = track(ctx);
        for (index, value) in self.values.iter().enumerate() {
            let x = bounds.x() + pitch * index as f32 + gap / 2.0;
            ctx.draw_quad(
                Quad::new(Rect::new(x, bounds.y(), width, bounds.height()))
                    .fill(track)
                    .radius(round),
            );
            let tall = bounds.height() * value.max(LEAST);
            ctx.draw_quad(
                Quad::new(Rect::new(x, bounds.max_y() - tall, width, tall))
                    .fill(self.color)
                    .radius(round),
            );
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.values == other.values && self.color == other.color && self.style == other.style
    }
}
