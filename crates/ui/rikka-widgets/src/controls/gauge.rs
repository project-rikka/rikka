use std::f32::consts::TAU;

use rikka_types::{Color, Size};
use rikka_ui::{Layout, Length, MeasureSpec, PaintCtx, Ring, Style, Widget, motion};

const DEFAULT_SIZE: f32 = 20.0;

const THICKNESS_RATIO: f32 = 0.16;

const TRACK_ALPHA: f32 = 0.16;

const LEAST: f32 = 0.02;

#[must_use]
#[derive(Widget)]
pub struct Gauge {
    #[debug]
    value: f32,
    size: f32,
    color: Option<Color>,
    #[style]
    style: Style,
}

pub fn gauge(value: f32) -> Gauge {
    Gauge {
        value: value.clamp(0.0, 1.0),
        size: DEFAULT_SIZE,
        color: None,
        style: Style::default(),
    }
}

impl Gauge {
    pub fn extent(mut self, extent: impl Into<Length>) -> Self {
        self.size = extent.into().resolve(0.0).unwrap_or(self.size).max(1.0);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl Widget for Gauge {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        Some(MeasureSpec::Fixed(Size::new(self.size, self.size)))
    }

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let color = self.color.unwrap_or(ctx.theme().palette().accent);
        let track = ctx.env().text_color.scale_alpha(TRACK_ALPHA);

        let (ring, width) = Ring::inside(ctx.bounds(), THICKNESS_RATIO);
        ctx.draw_ring(ring.stroke(width, track));

        let filled = ctx.animate_curve("filled", self.value, motion::FILL);
        if filled <= 0.0 {
            return;
        }
        ctx.draw_ring(
            ring.stroke(width, color)
                .sweep(Ring::TOP, filled.max(LEAST) * TAU),
        );
    }

    fn same(&self, other: &Self) -> bool {
        self.value == other.value
            && self.size == other.size
            && self.color == other.color
            && self.style == other.style
    }
}
