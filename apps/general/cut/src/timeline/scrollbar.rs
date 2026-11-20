use rikka_app::prelude::*;
use rikka_timeline::{Orientation, ScrollTrack, paint_thumb};

use crate::timeline::scale::Scale;

pub(crate) const HEIGHT: f32 = 12.0;

#[derive(Debug)]
pub(crate) struct Scrollbar {
    pub(crate) scale: Scale,
}

#[derive(Debug, Default)]
pub(crate) struct PointerState {
    grip: Option<f32>,
}

impl Scrollbar {
    fn track(&self, bounds: Rect) -> ScrollTrack {
        ScrollTrack {
            extent: self.scale.extent(),
            from: bounds.x(),
            length: bounds.width(),
        }
    }
}

impl Widget for Scrollbar {
    type State = PointerState;

    fn paint(&self, state: &mut PointerState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        ctx.draw_quad(Quad::new(bounds).fill(palette().surface));
        let track = self.track(bounds);
        if !track.extent.scrolls() {
            return;
        }
        let lit = state.grip.is_some() || ctx.hovered();
        paint_thumb(ctx, bounds, Orientation::Horizontal, track.thumb(), lit);
    }

    fn event(&self, state: &mut PointerState, event: &Event, ctx: &mut EventCtx<'_>) {
        let track = self.track(ctx.bounds());
        match *event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                let grip = track.grip(position.x);
                state.grip = Some(grip);
                self.scale.scroll_to(track.at_for(position.x - grip));
                ctx.capture_pointer();
                ctx.keep_focus();
                ctx.set_handled();
                ctx.request_paint();
            }
            Event::PointerMoved(position) => {
                if let Some(grip) = state.grip {
                    self.scale.scroll_to(track.at_for(position.x - grip));
                    ctx.set_handled();
                }
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } if state.grip.is_some() => {
                state.grip = None;
                ctx.release_pointer();
                ctx.set_handled();
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &PointerState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Scrollbar) -> bool {
        self.scale == other.scale
    }
}

impl View for Scrollbar {
    fn build(self) -> Element {
        Element::new(self).height(HEIGHT)
    }
}
