use std::path::Path;
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_types::PointerButton;
use rikka_ui::{Action, DRAG_SLOP, DragStart, Event, EventCtx, Offer};
use rikka_widgets::{Grab, GrabStep, drag_chip};

pub(super) struct Lift {
    child: Option<Element>,
    path: Arc<Path>,
    label: Arc<str>,
    symbol: MaterialSymbol,
    press: Action<()>,
    lifted: Action<()>,
}

#[derive(Debug, Default)]
pub(super) struct LiftState {
    grab: Grab<Point>,
}

pub(super) fn lift(
    child: impl View,
    path: Arc<Path>,
    label: Arc<str>,
    symbol: MaterialSymbol,
) -> Lift {
    Lift {
        child: Some(child.build()),
        path,
        label,
        symbol,
        press: Action::default(),
        lifted: Action::default(),
    }
}

impl Lift {
    pub(super) fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Action::new(press);
        self
    }

    pub(super) fn on_lift<Marker>(mut self, lifted: impl IntoHandler<(), (), Marker>) -> Self {
        self.lifted = Action::new(lifted);
        self
    }

    fn start(&self, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.lifted, ());
        let uri = rikka_open::path_to_uri(&self.path);
        let offer = Offer::uris(&[uri]).with_text(self.path.to_string_lossy());
        let icon = drag_chip(ctx.theme().clone(), self.symbol, &self.label);
        ctx.start_drag(DragStart::new(offer, icon));
    }
}

impl Widget for Lift {
    type State = LiftState;

    fn layout(&self) -> Layout {
        Layout::column()
    }

    fn event(&self, state: &mut LiftState, event: &Event, ctx: &mut EventCtx<'_>) {
        if let Some(step) = state.grab.update(event, ctx) {
            match step {
                GrabStep::Moved { held, to } if to.distance(held) > DRAG_SLOP => {
                    state.grab.end(ctx);
                    self.start(ctx);
                }
                GrabStep::Ended { .. } => ctx.act(&self.press, ()),
                GrabStep::Moved { .. } | GrabStep::Cancelled { .. } => {}
            }
            return;
        }
        if let Event::PointerPressed {
            button: PointerButton::Primary,
            position,
            ..
        } = *event
            && ctx.bounds().contains(position)
        {
            state.grab.begin(position, ctx);
        }
    }

    fn behavior(&self, _state: &LiftState) -> Behavior {
        Behavior::TRACKS_HOVER
    }

    fn same(&self, other: &Self) -> bool {
        self.path == other.path
            && self.label == other.label
            && self.symbol == other.symbol
            && self.press == other.press
            && self.lifted == other.lifted
    }
}

impl View for Lift {
    fn build(mut self) -> Element {
        let children = self.child.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl std::fmt::Debug for Lift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lift")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
