use std::ops::RangeInclusive;

use rikka_types::{Color, Key, Point, Rect};
use rikka_ui::{Action, PaintCtx, Quad};

pub(crate) const TRACK_WIDTH: f32 = 112.0;

const TRACK_THICKNESS: f32 = 4.0;
const KNOB: f32 = 14.0;
const NUDGE_FRACTION: f32 = 0.05;

#[derive(Clone, Debug)]
pub(crate) struct MenuSlider {
    pub(crate) value: f32,
    range: RangeInclusive<f32>,
    pub(crate) step: Option<f32>,
    pub(crate) change: Action<(f32,)>,
}

impl MenuSlider {
    pub(crate) fn new(value: f32, range: RangeInclusive<f32>) -> Self {
        Self {
            value,
            range,
            step: None,
            change: Action::default(),
        }
    }

    fn span(&self) -> f32 {
        self.range.end() - self.range.start()
    }

    fn fraction(&self) -> f32 {
        let span = self.span();
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.range.start()) / span).clamp(0.0, 1.0)
    }

    fn snapped(&self, value: f32) -> f32 {
        let snapped = match self.step {
            Some(step) if step > 0.0 => {
                self.range.start() + ((value - self.range.start()) / step).round() * step
            }
            _ => value,
        };
        snapped.clamp(*self.range.start(), *self.range.end())
    }

    pub(crate) fn track(row: Rect, inset: f32) -> Rect {
        let width = TRACK_WIDTH.min((row.width() - inset * 2.0).max(0.0));
        Rect::new(row.max_x() - inset - width, row.y(), width, row.height())
    }

    pub(crate) fn value_at(&self, track: Rect, x: f32) -> f32 {
        let travel = track.width() - KNOB;
        if travel <= 0.0 {
            return self.value;
        }
        let fraction = ((x - track.x() - KNOB / 2.0) / travel).clamp(0.0, 1.0);
        self.snapped(self.range.start() + fraction * self.span())
    }

    pub(crate) fn keyed(&self, key: Key) -> Option<f32> {
        let nudge = self
            .step
            .filter(|step| *step > 0.0)
            .unwrap_or(self.span() * NUDGE_FRACTION);
        let value = match key {
            Key::ArrowLeft => self.value - nudge,
            Key::ArrowRight => self.value + nudge,
            _ => return None,
        };
        Some(self.snapped(value))
    }

    pub(crate) fn paint(&self, ctx: &mut PaintCtx<'_>, track: Rect, enabled: bool) {
        let theme = ctx.theme();
        let palette = theme.palette();
        let dim = |color: Color| theme.dim(color, !enabled);
        let rail = dim(palette.surface_variant);
        let fill = dim(palette.accent);
        let knob = dim(theme.foreground_on(palette.accent));
        let outline = palette.outline;

        let center_y = track.center().y;
        let line = Rect::new(
            track.x(),
            center_y - TRACK_THICKNESS / 2.0,
            track.width(),
            TRACK_THICKNESS,
        );
        let radius = TRACK_THICKNESS / 2.0;
        ctx.draw_quad(Quad::new(line).fill(rail).radius(radius));
        let travel = (track.width() - KNOB).max(0.0);
        let reached = KNOB / 2.0 + travel * self.fraction();
        ctx.draw_quad(
            Quad::new(Rect::new(line.x(), line.y(), reached, TRACK_THICKNESS))
                .fill(fill)
                .radius(radius),
        );
        let center = Point::new(track.x() + reached, center_y);
        ctx.draw_quad(
            Quad::new(Rect::from_circle(center, KNOB / 2.0))
                .fill(knob)
                .stroke(1.0, outline)
                .radius(KNOB / 2.0),
        );
    }
}
