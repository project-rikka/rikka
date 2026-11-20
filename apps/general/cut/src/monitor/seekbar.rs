use std::time::Instant;

use rikka_app::prelude::*;

use crate::cut::Cut;
use crate::transport::Transport;

const HEIGHT: f32 = 24.0;
const TRACK: f32 = 4.0;
const KNOB: f32 = 12.0;
const REST: f32 = 0.25;

#[derive(Debug)]
pub(crate) struct Seekbar {
    pub(crate) transport: Transport,
    pub(crate) end: i64,
}

#[derive(Debug, Default)]
pub(crate) struct PointerState {
    seeking: bool,
}

impl Seekbar {
    fn track(bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + KNOB / 2.0,
            bounds.center().y - TRACK / 2.0,
            (bounds.width() - KNOB).max(0.0),
            TRACK,
        )
    }

    fn frame_at(&self, bounds: Rect, x: f32) -> i64 {
        let track = Self::track(bounds);
        if track.width() <= 0.0 {
            return 0;
        }
        let along = ((x - track.x()) / track.width()).clamp(0.0, 1.0);
        (f64::from(along) * self.end as f64).round() as i64
    }

    fn seek(&self, ctx: &mut EventCtx<'_>, x: f32) {
        let frame = self.frame_at(ctx.bounds(), x);
        ctx.run(move |cut: &mut Cut| cut.go_to(frame));
    }
}

impl Widget for Seekbar {
    type State = PointerState;

    fn paint(&self, _state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        if self.transport.is_playing() {
            ctx.request_frame();
        }
        let palette = palette();
        let track = Self::track(ctx.bounds());
        ctx.draw_quad(
            Quad::new(track)
                .fill(palette.text.scale_alpha(REST))
                .radius(TRACK / 2.0),
        );
        if self.end <= 0 {
            return;
        }
        let frame = self.transport.exact(Instant::now());
        let along = (frame / self.end as f64).clamp(0.0, 1.0) as f32;
        let x = track.x() + track.width() * along;
        ctx.draw_quad(
            Quad::new(Rect::new(track.x(), track.y(), x - track.x(), TRACK))
                .fill(palette.accent)
                .radius(TRACK / 2.0),
        );
        ctx.draw_circle(Point::new(x, track.center().y), KNOB, palette.accent);
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } if self.end > 0 => {
                state.seeking = true;
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                self.seek(ctx, position.x);
            }
            Event::PointerMoved(position) if state.seeking => {
                ctx.set_handled();
                self.seek(ctx, position.x);
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.seeking => {
                state.seeking = false;
                ctx.release_pointer();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn same(&self, other: &Seekbar) -> bool {
        self.transport == other.transport && self.end == other.end
    }
}

impl View for Seekbar {
    fn build(self) -> Element {
        Element::new(self).height(HEIGHT)
    }
}
