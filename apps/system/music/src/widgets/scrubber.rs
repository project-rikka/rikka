use std::time::{Duration, Instant};

use music_core::Clock;
use rikka_app::prelude::*;
use rikka_ui::{Handler, TextRun, motion};

use rikka_format::format_duration;

const THICKNESS: f32 = 4.0;
const HEIGHT: f32 = 24.0;
const LABEL: f32 = 46.0;
const LABEL_TEXT: f32 = 11.5;
const MIN_WAKE: Duration = Duration::from_millis(16);
const MAX_WAKE: Duration = Duration::from_millis(500);
const STEP: f64 = 0.5;

pub(crate) struct Scrubber {
    clock: Clock,
    on_seek: Option<Handler<(f32,)>>,
    style: Style,
}

pub(crate) fn scrubber(clock: Clock) -> Scrubber {
    Scrubber {
        clock,
        on_seek: None,
        style: Style::default(),
    }
}

impl Scrubber {
    pub(crate) fn on_seek<M>(mut self, on_seek: impl IntoHandler<(f32,), (), M>) -> Self {
        self.on_seek = Some(on_seek.into_handler());
        self
    }

    fn lane(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + LABEL,
            bounds.y(),
            (bounds.width() - LABEL * 2.0).max(0.0),
            bounds.height(),
        )
    }

    fn fraction_at(&self, bounds: Rect, x: f32) -> f32 {
        let lane = self.lane(bounds);
        if lane.width() <= 0.0 {
            return 0.0;
        }
        ((x - lane.x()) / lane.width()).clamp(0.0, 1.0)
    }

    fn wake(&self, lane: Rect, now: Instant) -> Option<Instant> {
        if !self.clock.running() || self.clock.length().is_zero() {
            return None;
        }
        let per_second = f64::from(lane.width()) / self.clock.length().as_secs_f64();
        let moved = match per_second > 0.0 {
            true => Duration::from_secs_f64(STEP / per_second),
            false => MAX_WAKE,
        };
        let position = self.clock.position(now);
        let tick =
            Duration::from_secs(1) - Duration::from_nanos(u64::from(position.subsec_nanos()));
        Some(now + moved.min(tick).clamp(MIN_WAKE, MAX_WAKE))
    }
}

#[derive(Debug, Default)]
pub(crate) struct ScrubberState {
    dragging: Option<f32>,
}

impl Widget for Scrubber {
    type State = ScrubberState;

    fn layout(&self) -> Layout {
        Layout::row()
            .size(Length::FILL, Length::px(HEIGHT))
            .fixed()
            .with(&self.style)
    }

    fn paint(&self, state: &mut ScrubberState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let now = ctx.now();
        let palette = ctx.palette();
        let lane = self.lane(bounds);
        let engaged = ctx.hovered() || state.dragging.is_some();
        let progress = state
            .dragging
            .unwrap_or_else(|| self.clock.progress(now))
            .clamp(0.0, 1.0);

        let swell = ctx.animate_curve("swell", if engaged { 1.5 } else { 1.0 }, motion::MORPH);
        let thickness = THICKNESS * swell;
        let track = Rect::new(
            lane.x(),
            lane.center().y - thickness / 2.0,
            lane.width(),
            thickness,
        );
        ctx.draw_quad(
            Quad::new(track)
                .fill(palette.surface_variant)
                .radius(thickness / 2.0),
        );
        let filled = Rect::new(track.x(), track.y(), track.width() * progress, thickness);
        ctx.draw_quad(
            Quad::new(filled)
                .fill(palette.accent)
                .radius(thickness / 2.0),
        );

        let knob = ctx.animate_curve("knob", if engaged { 12.0 } else { 0.0 }, motion::MORPH);
        if knob > 0.5 {
            let center = Point::new(filled.max_x(), track.center().y);
            ctx.draw_quad(
                Quad::new(Rect::from_circle(center, knob / 2.0))
                    .fill(palette.on_accent)
                    .stroke(1.0, palette.outline)
                    .radius(knob / 2.0),
            );
        }

        let length = self.clock.length();
        let shown = match state.dragging {
            Some(fraction) => length.mul_f32(fraction),
            None => self.clock.position(now),
        };
        let size = LABEL_TEXT;
        let elapsed = TextRun::new(format_duration(shown), size).no_wrap();
        let total = TextRun::new(format_duration(length), size).no_wrap();
        let line = measure_text(&elapsed, None).height;
        let y = bounds.center().y - line / 2.0;
        ctx.draw_text(
            Rect::new(bounds.x(), y, LABEL - 8.0, line),
            elapsed,
            palette.text_muted,
        );
        let width = measure_text(&total, None).width;
        ctx.draw_text(
            Rect::new(bounds.max_x() - width, y, width, line),
            total,
            palette.text_muted,
        );

        if state.dragging.is_none()
            && let Some(at) = self.wake(lane, now)
        {
            ctx.wake_at(at);
        }
    }

    fn event(&self, state: &mut ScrubberState, event: &Event, ctx: &mut EventCtx<'_>) {
        let Some(on_seek) = self.on_seek.as_ref() else {
            return;
        };
        let bounds = ctx.bounds();
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                state.dragging = Some(self.fraction_at(bounds, position.x));
                ctx.capture_pointer();
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::PointerMoved(position) if state.dragging.is_some() => {
                state.dragging = Some(self.fraction_at(bounds, position.x));
                ctx.request_paint();
                ctx.set_handled();
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if let Some(fraction) = state.dragging.take() {
                    ctx.release_pointer();
                    ctx.request_paint();
                    ctx.set_handled();
                    ctx.call(on_seek, (fraction,));
                }
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &ScrubberState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Self) -> bool {
        self.clock == other.clock
            && self.on_seek.is_some() == other.on_seek.is_some()
            && self.style == other.style
    }

    fn debug_name(&self) -> &'static str {
        "Scrubber"
    }
}

impl View for Scrubber {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl Styled for Scrubber {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl std::fmt::Debug for Scrubber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Scrubber")
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}
