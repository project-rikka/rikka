use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_scrolling::{Edge, Frame, Sizing, Strip};

use rikka_compositor_tiling::{Gaps, Tree};

use crate::policy::LayoutMode;
use rikka_compositor_tiling::Edges;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Dwindle,
    Columns,
}

impl Kind {
    pub(crate) fn from_mode(mode: LayoutMode) -> Self {
        match mode {
            LayoutMode::Scrolling => Self::Columns,
            LayoutMode::Floating | LayoutMode::Tiling => Self::Dwindle,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Layout {
    Dwindle(Tree<WindowId>),
    Columns(Strip<WindowId>),
}

impl Layout {
    pub(crate) fn new(kind: Kind, frame: Frame) -> Self {
        match kind {
            Kind::Dwindle => Self::Dwindle(Tree::default()),
            Kind::Columns => Self::Columns(Strip::new(frame)),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Dwindle(tree) => tree.is_empty(),
            Self::Columns(strip) => strip.is_empty(),
        }
    }

    pub(crate) fn contains(&self, window: WindowId) -> bool {
        match self {
            Self::Dwindle(tree) => tree.contains(&window),
            Self::Columns(strip) => strip.contains(&window),
        }
    }

    pub(crate) fn windows(&self) -> Vec<WindowId> {
        match self {
            Self::Dwindle(tree) => tree.windows(),
            Self::Columns(strip) => strip.windows().collect(),
        }
    }

    pub(crate) fn strip(&self) -> Option<&Strip<WindowId>> {
        match self {
            Self::Columns(strip) => Some(strip),
            Self::Dwindle(_) => None,
        }
    }

    pub(crate) fn strip_mut(&mut self) -> Option<&mut Strip<WindowId>> {
        match self {
            Self::Columns(strip) => Some(strip),
            Self::Dwindle(_) => None,
        }
    }

    pub(crate) fn insert(&mut self, frame: Frame, window: WindowId, next_to: Option<WindowId>) {
        match self {
            Self::Dwindle(tree) => tree.insert(frame.work, window, next_to.as_ref()),
            Self::Columns(strip) => {
                strip.reframe(frame);
                strip.insert(window, next_to.as_ref());
            }
        }
    }

    pub(crate) fn remove(&mut self, window: WindowId) -> bool {
        match self {
            Self::Dwindle(tree) => tree.remove(&window),
            Self::Columns(strip) => strip.remove(&window),
        }
    }

    pub(crate) fn enlarge(&mut self, window: WindowId, sizing: Sizing) -> bool {
        match self {
            Self::Dwindle(_) => false,
            Self::Columns(strip) => strip.enlarge(&window, sizing),
        }
    }

    pub(crate) fn arrival(&self, frame: Frame, next_to: Option<WindowId>) -> Rect<i32, Logical> {
        match self {
            Self::Dwindle(tree) => tree.arrival(frame.work, gaps(frame), next_to.as_ref()),
            Self::Columns(strip) => strip.arrival(frame, next_to.as_ref()),
        }
    }

    pub(crate) fn arrange(&mut self, frame: Frame) -> Vec<(WindowId, Rect<i32, Logical>)> {
        if let Self::Columns(strip) = self {
            strip.reframe(frame);
            strip.settle();
        }
        self.tiles(frame)
    }

    pub(crate) fn tiles(&self, frame: Frame) -> Vec<(WindowId, Rect<i32, Logical>)> {
        match self {
            Self::Dwindle(tree) => tree.tiles(frame.work, gaps(frame)),
            Self::Columns(strip) => strip.tiles(),
        }
    }

    pub(crate) fn window_at(&self, frame: Frame, at: Point<f64, Logical>) -> Option<WindowId> {
        self.tiles(frame)
            .into_iter()
            .find(|(_, tile)| tile.to_f64().contains(at))
            .map(|(window, _)| window)
    }

    pub(crate) fn edge_at(
        &self,
        frame: Frame,
        at: Point<f64, Logical>,
        reach: f64,
    ) -> Option<(WindowId, Edges)> {
        self.tiles(frame)
            .into_iter()
            .map(|(window, tile)| (window, Edges::near(tile, at, reach)))
            .find(|(_, edges)| !edges.is_empty())
    }

    pub(crate) fn resize(
        &mut self,
        frame: Frame,
        window: WindowId,
        edges: Edges,
        delta: Point<f64, Logical>,
    ) -> bool {
        match self {
            Self::Dwindle(tree) => tree.resize(frame.work, gaps(frame), &window, edges, delta),
            Self::Columns(strip) => [
                (Edges::LEFT, Edge::Left, delta.x),
                (Edges::RIGHT, Edge::Right, delta.x),
                (Edges::TOP, Edge::Top, delta.y),
                (Edges::BOTTOM, Edge::Bottom, delta.y),
            ]
            .into_iter()
            .filter(|(edge, _, _)| edges.contains(*edge))
            .fold(false, |moved, (_, edge, by)| {
                strip.resize(&window, edge, by) | moved
            }),
        }
    }

    pub(crate) fn rearrange(&mut self, frame: Frame, window: WindowId, at: Point<f64, Logical>) {
        if let Self::Columns(strip) = self {
            strip.drop_at(&window, at);
            return;
        }
        let onto = self.window_at(frame, at).filter(|onto| *onto != window);
        if let (Some(onto), Self::Dwindle(tree)) = (onto, self) {
            tree.swap(&window, &onto);
        }
    }
}

fn gaps(frame: Frame) -> Gaps {
    Gaps {
        inner: frame.spacing.inner,
        outer: frame.spacing.outer,
    }
}
