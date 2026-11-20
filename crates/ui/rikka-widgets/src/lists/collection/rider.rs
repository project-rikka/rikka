use rikka_types::{Point, Rect, Size};
use rikka_ui::{Axis, Behavior, Element, Layout, Length, PaintCtx, Widget};

use crate::blocks::cell_content::{CellContent, Lazily};
use crate::lists::collection::ride::Riders;

fn snap(home: f32) -> f32 {
    (home + 0.5).floor()
}

pub(crate) struct Rider {
    riders: Riders,
    riding: Option<u64>,
    id: u64,
    along: Axis,
    home: f32,
    placed: Option<(f32, f32)>,
    lazily: Option<Lazily>,
}

impl Rider {
    pub(crate) fn new(
        riders: &Riders,
        riding: Option<u64>,
        id: u64,
        along: Axis,
        home: f32,
    ) -> Rider {
        Rider {
            riders: riders.clone(),
            riding,
            id,
            along,
            home,
            placed: None,
            lazily: None,
        }
    }

    pub(crate) fn placed(mut self, across: f32, breadth: f32) -> Rider {
        self.placed = Some((across, breadth));
        self
    }

    fn snapped(&self) -> f32 {
        self.home - snap(self.home)
    }

    pub(crate) fn with_content(mut self, content: CellContent) -> Element {
        let (children, lazily) = content.parts();
        self.lazily = lazily;
        Element::with_children(self, children)
    }
}

#[derive(Debug, Default)]
pub(crate) struct RiderState {
    drift: f32,
}

impl Widget for Rider {
    type State = RiderState;

    fn behavior(&self, _state: &RiderState) -> Behavior {
        Behavior::TRAVELS | Behavior::PAINTS_OFFSCREEN.when(self.riding.is_some())
    }

    fn layout(&self) -> Layout {
        let Some((across, breadth)) = self.placed else {
            return Layout::DEFAULT;
        };
        let layout = Layout::DEFAULT.placed_at(self.along.point(snap(self.home), across));
        match self.along {
            Axis::Vertical => layout.width(Length::px(breadth)),
            Axis::Horizontal => layout.height(Length::px(breadth)),
        }
    }

    fn paint(&self, state: &mut RiderState, ctx: &mut PaintCtx<'_>) {
        let Some(shown) = self.riders.shown(self.id) else {
            state.drift = 0.0;
            if self.placed.is_some() {
                let bounds = ctx.bounds();
                ctx.set_children_clip(Rect::new(bounds.x(), bounds.y(), 0.0, 0.0));
            }
            return;
        };
        ctx.request_frame();
        let snapped = self.snapped();
        state.drift = shown.drift + snapped;
        let bounds = ctx.bounds();
        let far = self.along.room(bounds) + state.drift.abs();
        let from = (shown.from + snapped).max(-far);
        ctx.set_children_clip(self.along.rect(
            bounds.origin,
            from,
            (far - from).max(0.0),
            0.0,
            self.along.across().room(bounds),
        ));
    }

    fn children_offset(&self, state: &RiderState, _bounds: Rect, _content: Size) -> Point {
        self.along.point(state.drift, 0.0)
    }

    fn hit_test(&self, _bounds: Rect, _point: Point) -> bool {
        false
    }

    fn build(&self, _state: &mut RiderState) -> Vec<Element> {
        self.lazily.iter().map(Lazily::build).collect()
    }

    fn memo(&self) -> Option<u64> {
        self.lazily.as_ref().map(Lazily::digest)
    }

    fn same(&self, other: &Self) -> bool {
        self.riders.is(&other.riders)
            && self.riding == other.riding
            && self.id == other.id
            && self.along == other.along
            && self.placed == other.placed
            && (self.riding.is_none() || self.home == other.home)
            && self.memo() == other.memo()
    }
}

impl std::fmt::Debug for Rider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rider")
            .field("id", &self.id)
            .field("placed", &self.placed)
            .finish_non_exhaustive()
    }
}
