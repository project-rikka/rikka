use std::ops::RangeInclusive;

use rikka_types::{Color, Key, Point, PointerButton, Rect};
use rikka_ui::{
    Action, Behavior, Event, EventCtx, IntoHandler, Layout, Length, PaintCtx, Quad, Style, Widget,
    motion,
};

use crate::control::ControlSize;
use crate::controls::grab::{Grab, GrabStep};
use crate::controls::ripple::engaged;

const KNOB_GRIP: f32 = 0.82;
const TRACK_SWELL: f32 = 1.35;

const PAGE_FRACTION: f32 = 0.1;

const ARROW_FRACTION: f32 = 0.01;

const TICK: f32 = 4.0;
const TICK_ALPHA: f32 = 0.35;

#[must_use]
#[derive(Widget)]
pub struct Slider {
    #[debug]
    value: f32,
    range: RangeInclusive<f32>,
    step: Option<f32>,
    ticks: Option<usize>,
    size: ControlSize,
    color: Option<Color>,
    on_change: Action<(f32,)>,
    on_commit: Action<(f32,)>,
    enabled: bool,
    #[style]
    style: Style,
}

pub fn slider(value: f32) -> Slider {
    Slider {
        value,
        range: 0.0..=1.0,
        step: None,
        ticks: None,
        size: ControlSize::default(),
        color: None,
        on_change: Action::default(),
        on_commit: Action::default(),
        enabled: true,
        style: Style::default(),
    }
}

impl Slider {
    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step.abs());
        self
    }

    pub fn ticks(mut self, count: usize) -> Self {
        self.ticks = Some(count);
        self
    }

    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn on_change<Marker>(mut self, on_change: impl IntoHandler<(f32,), (), Marker>) -> Self {
        self.on_change = Action::new(on_change);
        self
    }

    pub fn on_commit<Marker>(mut self, on_commit: impl IntoHandler<(f32,), (), Marker>) -> Self {
        self.on_commit = Action::new(on_commit);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn is_active(&self) -> bool {
        self.on_change.active(self.enabled) || self.on_commit.active(self.enabled)
    }

    fn knob(&self) -> f32 {
        match self.size {
            ControlSize::Small => 20.0,
            ControlSize::Medium => 26.0,
            ControlSize::Large => 30.0,
        }
    }

    fn track(&self) -> f32 {
        match self.size {
            ControlSize::Small => 4.0,
            ControlSize::Medium => 6.0,
            ControlSize::Large => 8.0,
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

    fn value_at(&self, fraction: f32) -> f32 {
        self.snapped(self.range.start() + fraction.clamp(0.0, 1.0) * self.span())
    }

    fn nudge(&self) -> f32 {
        self.step
            .filter(|step| *step > 0.0)
            .unwrap_or(self.span() * ARROW_FRACTION)
    }

    fn shifted(&self, delta: f32) -> f32 {
        (self.value + delta).clamp(*self.range.start(), *self.range.end())
    }

    fn fraction_at(&self, bounds: Rect, x: f32) -> f32 {
        let knob = self.knob();
        let travel = bounds.width() - knob;
        if travel <= 0.0 {
            return 0.0;
        }
        ((x - bounds.x() - knob / 2.0) / travel).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct SliderState {
    grab: Grab,
}

impl Widget for Slider {
    type State = SliderState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(self.knob()))
            .fixed()
            .with(&self.style)
    }

    fn paint(&self, state: &mut SliderState, ctx: &mut PaintCtx<'_>) {
        let knob = self.knob();
        let bounds = ctx.bounds();
        let center_y = bounds.center().y;
        let engaged = engaged(ctx, self.is_active(), state.grab.is_held());

        let palette = ctx.theme().palette();
        let (fill_color, knob_color, track_color, tick_tint) = {
            let theme = ctx.theme();
            let dim = |color| theme.dim(color, !self.enabled);
            let track = dim(palette.surface_variant);
            let fill = self.color.unwrap_or(palette.accent);
            (
                dim(fill),
                dim(theme.foreground_on(fill)),
                track,
                theme.foreground_on(track).scale_alpha(TICK_ALPHA),
            )
        };

        let offset = ctx
            .steer("offset", self.fraction(), state.grab.is_held())
            .clamp(0.0, 1.0);

        let swell = ctx.animate_curve(
            "swell",
            if engaged { TRACK_SWELL } else { 1.0 },
            motion::MORPH,
        );
        let grip = ctx.animate_curve(
            "grip",
            if state.grab.is_held() { KNOB_GRIP } else { 1.0 },
            motion::MORPH,
        );

        let thickness = self.track() * swell;
        let track = Rect::new(
            bounds.x(),
            center_y - thickness / 2.0,
            bounds.width(),
            thickness,
        );
        ctx.draw_quad(Quad::new(track).fill(track_color).radius(thickness / 2.0));

        let travel = (bounds.width() - knob).max(0.0);
        let filled = knob / 2.0 + travel * offset;
        ctx.draw_quad(
            Quad::new(Rect::new(track.x(), track.y(), filled, thickness))
                .fill(fill_color)
                .radius(thickness / 2.0),
        );

        if let Some(count) = self.ticks.filter(|count| *count > 1) {
            for index in 0..count {
                let at = index as f32 / (count - 1) as f32;
                let x = track.x() + knob / 2.0 + travel * at;
                ctx.draw_circle(Point::new(x, center_y), TICK, tick_tint);
            }
        }

        let center = Point::new(bounds.x() + knob / 2.0 + travel * offset, center_y);
        let gripped = knob * grip;

        ctx.draw_quad(
            Quad::new(Rect::from_circle(center, gripped / 2.0))
                .fill(knob_color)
                .stroke(1.0, palette.outline)
                .radius(gripped / 2.0),
        );
    }

    fn event(&self, state: &mut SliderState, event: &Event, ctx: &mut EventCtx<'_>) {
        if !self.is_active() {
            state.grab.end(ctx);
            return;
        }
        let bounds = ctx.bounds();
        let change = |ctx: &mut EventCtx<'_>, value: f32| {
            if value != self.value {
                ctx.act(&self.on_change, (value,));
            }
        };
        let pointed = |x: f32| self.value_at(self.fraction_at(bounds, x));

        match state.grab.update(event, ctx) {
            Some(GrabStep::Moved { to, .. }) => return change(ctx, pointed(to.x)),
            Some(GrabStep::Ended { at, .. }) => {
                return ctx.act(&self.on_commit, (pointed(at.x),));
            }
            Some(GrabStep::Cancelled { .. }) => {
                return ctx.act(&self.on_commit, (self.value,));
            }
            None => {}
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                state.grab.begin((), ctx);
                change(ctx, pointed(position.x));
            }
            Event::KeyPressed { key, .. } if ctx.focused() => {
                let value = match key {
                    Key::ArrowLeft | Key::ArrowDown => self.shifted(-self.nudge()),
                    Key::ArrowRight | Key::ArrowUp => self.shifted(self.nudge()),
                    Key::PageDown => self.shifted(-self.span() * PAGE_FRACTION),
                    Key::PageUp => self.shifted(self.span() * PAGE_FRACTION),
                    Key::Home => *self.range.start(),
                    Key::End => *self.range.end(),
                    _ => return,
                };
                ctx.set_handled();
                let value = self.snapped(value);
                if value == self.value {
                    return;
                }
                change(ctx, value);
                ctx.act(&self.on_commit, (value,));
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::TRACKS_HOVER | Behavior::FOCUSABLE.when(self.is_active())
    }
}
