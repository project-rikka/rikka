use std::time::Instant;

use rikka_ui::{PaintCtx, Place, View, Widget, scroll::ScrollPosition};
use ui_scene::{Bounce, LIST_ROWS};

use crate::PROBE;
use crate::clock::Clock;

pub(crate) struct Ticker {
    clock: Clock,
    scrolled: Option<Place>,
}

#[derive(Debug)]
pub(crate) struct TickerState {
    shown: Option<Instant>,
    bounce: Bounce,
}

impl Default for TickerState {
    fn default() -> TickerState {
        TickerState {
            shown: None,
            bounce: Bounce::new(LIST_ROWS),
        }
    }
}

pub(crate) fn ticker(clock: Clock, scrolled: Option<Place>) -> Ticker {
    Ticker { clock, scrolled }
}

impl Widget for Ticker {
    type State = TickerState;

    fn paint(&self, state: &mut TickerState, ctx: &mut PaintCtx<'_>) {
        ctx.request_frame();
        let shown = ctx.now();
        if state.shown.replace(shown) == Some(shown) {
            return;
        }
        PROBE.frame();
        self.clock.tick();
        if let Some(place) = &self.scrolled {
            place.set(ScrollPosition {
                subject: Some(0),
                offset: state.bounce.advance(),
                ..ScrollPosition::default()
            });
        }
    }

    fn same(&self, _: &Self) -> bool {
        false
    }
}

impl std::fmt::Debug for Ticker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Ticker").finish_non_exhaustive()
    }
}

impl View for Ticker {
    fn build(self) -> rikka_ui::Element {
        rikka_ui::Element::new(self)
    }
}
