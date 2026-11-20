use rikka_types::{Color, Rect};
use rikka_ui::{Layout, PaintCtx, Quad, Style, Widget};

use crate::widgets::paint::track::track;

const GAP: f32 = 2.0;
const ROUND: f32 = 1.5;

#[derive(Widget)]
pub(crate) struct Segments {
    #[debug]
    level: f32,
    count: usize,
    color: Color,
    #[style]
    style: Style,
}

#[must_use]
pub(crate) fn segments(level: f32, count: usize, color: Color) -> Segments {
    Segments {
        level: level.clamp(0.0, 1.0),
        count: count.max(1),
        color,
        style: Style::default(),
    }
}

impl Widget for Segments {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let gaps = GAP * (self.count - 1) as f32;
        let cell = (bounds.width() - gaps) / self.count as f32;
        if cell <= 0.0 {
            return;
        }
        let track = track(ctx);
        let filled = self.level * self.count as f32;
        for index in 0..self.count {
            let x = bounds.x() + (cell + GAP) * index as f32;
            let whole = Rect::new(x, bounds.y(), cell, bounds.height());
            ctx.draw_quad(Quad::new(whole).fill(track).radius(ROUND));
            let share = (filled - index as f32).clamp(0.0, 1.0);
            if share > 0.0 {
                ctx.draw_quad(
                    Quad::new(Rect::new(x, bounds.y(), cell * share, bounds.height()))
                        .fill(self.color)
                        .radius(ROUND),
                );
            }
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.level == other.level
            && self.count == other.count
            && self.color == other.color
            && self.style == other.style
    }
}
