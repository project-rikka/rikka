use std::sync::Arc;

use rikka_session::wallpaper::Reserved;
use rikka_types::Rect;
use rikka_ui::{Element, Layout, Length, PaintCtx, View, Widget};

use crate::Picture;
use crate::backdrop::transition::{Stage, Transition};

#[derive(Debug)]
pub(crate) struct Crossfade {
    pub(crate) transition: Arc<dyn Transition>,
    pub(crate) outgoing: Option<Picture>,
    pub(crate) incoming: Picture,
    pub(crate) phase: Phase,
    pub(crate) generation: u64,
    pub(crate) reserved: Reserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Holding,
    Crossfade,
    Settled,
}

#[derive(Debug, Default)]
pub(crate) struct CrossfadeState {
    generation: u64,
}

const PROGRESS: &str = "crossing";

impl Widget for Crossfade {
    type State = CrossfadeState;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut Self::State, ctx: &mut PaintCtx<'_>) {
        if state.generation != self.generation {
            state.generation = self.generation;
            ctx.restart(PROGRESS);
        }
        let progress = match self.phase {
            Phase::Holding => 0.0,
            Phase::Crossfade => ctx.play(PROGRESS, self.transition.curve()),
            Phase::Settled => 1.0,
        };

        let within = within(ctx.bounds(), self.reserved);
        if within.is_empty() {
            return;
        }
        let mut stage = Stage::new(ctx, within, self.outgoing.as_ref(), &self.incoming);
        self.transition.paint(progress, &mut stage);
    }

    fn debug_name(&self) -> &'static str {
        "Wallpaper"
    }
}

impl View for Crossfade {
    fn build(self) -> Element {
        Element::new(self)
    }
}

fn within(bounds: Rect, reserved: Reserved) -> Rect {
    let left = reserved.left.clamp(0.0, bounds.width());
    let top = reserved.top.clamp(0.0, bounds.height());
    Rect::new(
        bounds.x() + left,
        bounds.y() + top,
        (bounds.width() - left - reserved.right.max(0.0)).max(0.0),
        (bounds.height() - top - reserved.bottom.max(0.0)).max(0.0),
    )
}
