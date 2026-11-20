use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use rikka_animation::Easing;
use rikka_types::{Color, Rect, Size};
use rikka_ui::{Layout, Length, MeasureSpec, PaintCtx, Ring, Style, Widget};

const DEFAULT_SIZE: f32 = 20.0;

const THICKNESS_RATIO: f32 = 0.115;

pub(crate) const TRACK_ALPHA: f32 = 0.10;

const PERIOD: Duration = Duration::from_millis(1600);
const TURNS: f32 = 2.0;

const JUMP: f32 = 0.66;

const MIN_SWEEP: f32 = 0.06;

const CREEP: f32 = TURNS - JUMP;

pub(crate) fn looped(ctx: &mut PaintCtx<'_>, since: &mut Option<Instant>, period: Duration) -> f32 {
    let started = *since.get_or_insert(ctx.now());
    let elapsed = ctx.now().saturating_duration_since(started);
    ctx.request_frame();
    (elapsed.as_nanos() % period.as_nanos()) as f32 / period.as_nanos() as f32
}

fn arc(phase: f32) -> (f32, f32) {
    let jump = |local: f32| JUMP * Easing::Standard.transform(local);
    let (head, tail) = (jump(phase * 2.0), jump(phase * 2.0 - 1.0));
    (CREEP * phase + tail, MIN_SWEEP + (head - tail))
}

pub(crate) fn spun(
    ctx: &mut PaintCtx<'_>,
    since: &mut Option<Instant>,
    bounds: Rect,
    color: Color,
    track: Color,
) {
    let (start, sweep) = arc(looped(ctx, since, PERIOD));
    let (ring, width) = Ring::inside(bounds, THICKNESS_RATIO);
    ctx.draw_ring(ring.stroke(width, track));
    ctx.draw_ring(
        ring.stroke(width, color)
            .sweep(Ring::TOP + start * TAU, sweep * TAU),
    );
}

#[must_use]
#[derive(Widget)]
pub struct Spinner {
    #[debug]
    size: f32,
    color: Option<Color>,
    #[style]
    style: Style,
}

pub fn spinner() -> Spinner {
    Spinner {
        size: DEFAULT_SIZE,
        color: None,
        style: Style::default(),
    }
}

impl Spinner {
    pub fn extent(mut self, extent: impl Into<Length>) -> Self {
        self.size = extent.into().resolve(0.0).unwrap_or(self.size).max(1.0);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct SpinnerState {
    since: Option<Instant>,
}

impl Widget for Spinner {
    type State = SpinnerState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT.with(&self.style)
    }

    fn measure(&self) -> Option<MeasureSpec> {
        Some(MeasureSpec::Fixed(Size::new(self.size, self.size)))
    }

    fn paint(&self, state: &mut SpinnerState, ctx: &mut PaintCtx<'_>) {
        let color = self.color.unwrap_or(ctx.env().text_color);
        let track = ctx.env().text_color.scale_alpha(TRACK_ALPHA);
        let bounds = ctx.bounds();

        spun(ctx, &mut state.since, bounds, color, track);
    }

    fn same(&self, other: &Self) -> bool {
        self.size == other.size && self.color == other.color && self.style == other.style
    }
}
