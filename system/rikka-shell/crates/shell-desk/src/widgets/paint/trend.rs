use rikka_types::{Color, Point};
use rikka_ui::{Layout, PaintCtx, Style, Widget};

use crate::widgets::paint::stroke::stroke;

const LINE: f32 = 1.5;
const DOT: f32 = 5.0;

#[derive(Widget)]
pub(crate) struct Trend {
    #[debug]
    values: Vec<f32>,
    color: Color,
    #[style]
    style: Style,
}

#[must_use]
pub(crate) fn trend(values: impl IntoIterator<Item = f32>, color: Color) -> Trend {
    Trend {
        values: values
            .into_iter()
            .map(|value| value.clamp(0.0, 1.0))
            .collect(),
        color,
        style: Style::default(),
    }
}

impl Widget for Trend {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        if self.values.is_empty() || bounds.height() <= DOT {
            return;
        }
        let pitch = bounds.width() / self.values.len() as f32;
        let reach = bounds.height() - DOT;
        let point = |index: usize, value: f32| {
            Point::new(
                bounds.x() + pitch * (index as f32 + 0.5),
                bounds.max_y() - DOT / 2.0 - reach * value,
            )
        };
        for (index, pair) in self.values.windows(2).enumerate() {
            stroke(
                ctx,
                point(index, pair[0]),
                point(index + 1, pair[1]),
                LINE,
                self.color,
            );
        }
        for (index, value) in self.values.iter().enumerate() {
            ctx.draw_circle(point(index, *value), DOT, self.color);
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.values == other.values && self.color == other.color && self.style == other.style
    }
}
