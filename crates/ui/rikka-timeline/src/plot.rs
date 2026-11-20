use rikka_types::{Color, Point, Rect};
use rikka_ui::PaintCtx;

const KNOB: f32 = 10.0;
const HELD_KNOB: f32 = 13.0;
const KNOB_CORE: f32 = 4.0;
const KNOB_REACH: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvePlot {
    area: Rect,
    low: f32,
    high: f32,
}

impl CurvePlot {
    #[must_use]
    pub fn unit(area: Rect) -> CurvePlot {
        CurvePlot {
            area,
            low: 0.0,
            high: 1.0,
        }
    }

    #[must_use]
    pub fn fitting(area: Rect, values: impl IntoIterator<Item = f32>, margin: f32) -> CurvePlot {
        let (low, high) = values
            .into_iter()
            .filter(|value| value.is_finite())
            .fold((0.0_f32, 1.0_f32), |(low, high), value| {
                (low.min(value), high.max(value))
            });
        let room = (high - low) * margin;
        CurvePlot {
            area,
            low: low - room,
            high: high + room,
        }
    }

    #[must_use]
    pub fn with_area(self, area: Rect) -> CurvePlot {
        CurvePlot { area, ..self }
    }

    #[must_use]
    pub fn area(&self) -> Rect {
        self.area
    }

    #[must_use]
    pub fn place(&self, point: Point) -> Point {
        let rise = (point.y - self.low) / (self.high - self.low);
        Point::new(
            self.area.x() + point.x * self.area.width(),
            self.area.max_y() - rise * self.area.height(),
        )
    }

    #[must_use]
    pub fn unplace(&self, at: Point) -> Point {
        let across = ((at.x - self.area.x()) / self.area.width()).clamp(0.0, 1.0);
        let rise = (self.area.max_y() - at.y) / self.area.height();
        Point::new(across, self.low + rise * (self.high - self.low))
    }

    pub fn line(&self, ctx: &mut PaintCtx<'_>, points: &[Point], width: f32, color: Color) {
        for pair in points.windows(2) {
            ctx.draw_segment(self.place(pair[0]), self.place(pair[1]), width, color);
        }
    }

    #[must_use]
    pub fn knob_at(&self, knobs: &[Point], pointer: Point) -> Option<usize> {
        knobs
            .iter()
            .enumerate()
            .map(|(index, knob)| (index, self.place(*knob).distance(pointer)))
            .filter(|(_, away)| *away <= KNOB_REACH)
            .min_by(|one, two| one.1.total_cmp(&two.1))
            .map(|(index, _)| index)
    }

    pub fn knob(&self, ctx: &mut PaintCtx<'_>, knob: Point, held: bool, color: Color) {
        let at = self.place(knob);
        let core = ctx.palette().surface;
        let size = match held {
            true => HELD_KNOB,
            false => KNOB,
        };
        ctx.draw_circle(at, size, color);
        ctx.draw_circle(at, KNOB_CORE, core);
    }
}
