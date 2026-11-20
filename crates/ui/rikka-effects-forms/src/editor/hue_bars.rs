use rikka_types::Color;
use rikka_ui::{Layout, Length, PaintCtx, Quad, Rect, Widget};

use super::plot::draw_ramp;

const BAR: f32 = 12.0;
const SPACING: f32 = 4.0;
const MARK: f32 = 3.0;
const SLICES: usize = 120;
const CORE: f32 = 15.0;
const FALLOFF: f32 = 45.0;

#[derive(Widget)]
pub(crate) struct HueBars {
    #[debug]
    shift: f32,
    range: Option<f32>,
}

impl HueBars {
    pub(crate) fn new(shift: f32) -> HueBars {
        HueBars { shift, range: None }
    }

    pub(crate) fn centered_on(mut self, center: Option<f32>) -> HueBars {
        self.range = center;
        self
    }
}

fn hue_color(degrees: f32) -> Color {
    Color::from_hsv(degrees / 360.0, 1.0, 1.0)
}

impl Widget for HueBars {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column()
            .size(Length::FILL, Length::px(BAR * 2.0 + SPACING + MARK))
            .fixed()
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let palette = ctx.palette();
        let bounds = ctx.bounds();
        let top = Rect::new(bounds.x(), bounds.y(), bounds.width(), BAR);
        let bottom = Rect::new(
            bounds.x(),
            top.max_y() + SPACING + MARK,
            bounds.width(),
            BAR,
        );
        let start = -180.0;
        draw_ramp(ctx, top, |along| hue_color(start + along * 360.0), SLICES);
        draw_ramp(
            ctx,
            bottom,
            |along| hue_color(start + along * 360.0 + self.shift),
            SLICES,
        );
        ctx.draw_quad(Quad::new(top).stroke(1.0, palette.outline));
        ctx.draw_quad(Quad::new(bottom).stroke(1.0, palette.outline));
        let Some(center) = self.range else {
            return;
        };
        let x_of = |degrees: f32| {
            let along = ((degrees - start).rem_euclid(360.0)) / 360.0;
            bounds.x() + along * bounds.width()
        };
        for (reach, color) in [(FALLOFF, palette.text_muted), (CORE, palette.text)] {
            let (from, to) = (x_of(center - reach), x_of(center + reach));
            let y = top.max_y() + SPACING / 2.0;
            let spans = match from <= to {
                true => vec![(from, to)],
                false => vec![(bounds.x(), to), (from, bounds.max_x())],
            };
            for (from, to) in spans {
                ctx.draw_quad(Quad::new(Rect::new(from, y, to - from, MARK)).fill(color));
            }
        }
    }

    fn same(&self, other: &HueBars) -> bool {
        self.shift == other.shift && self.range == other.range
    }
}
