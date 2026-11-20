use std::collections::HashMap;
use std::sync::Arc;

use rikka_compositor_desktop::{SnapshotId, Target, WindowId};
use rikka_compositor_geometry::{Logical, Point, Rect, Scale};
use rikka_compositor_surface::SurfaceId;
use rikka_renderer::{Node, Stroke};
use rikka_text::{Align, TextSpec, Wrap};
use rikka_types::{Color, Rect as SceneRect, RoundedRect};

use super::caption::CaptionPaint;
use super::outline::Outline;
use super::tree::Placement;
use super::window_shape::WindowShape;
use crate::glass::Frost;
use crate::wayland::ViewId;

const TINIEST: f64 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Visual {
    pub(crate) rect: Rect<f64, Logical>,
    pub(crate) alpha: f32,
}

impl Visual {
    pub(crate) fn opaque(geometry: Rect<i32, Logical>) -> Self {
        Self {
            rect: geometry.to_f64(),
            alpha: 1.0,
        }
    }

    pub(crate) fn shifted(mut self, by: f64) -> Self {
        self.rect.loc.x += by;
        self
    }

    pub(crate) fn faded(mut self, alpha: f32) -> Self {
        self.alpha *= alpha;
        self
    }

    pub(crate) fn stretch(self, geometry: Rect<i32, Logical>) -> Scale<f64> {
        let along = |shown: f64, standing: i32| match standing > 0 {
            true => (shown / f64::from(standing)).max(TINIEST),
            false => 1.0,
        };
        Scale {
            x: along(self.rect.size.w, geometry.size.w),
            y: along(self.rect.size.h, geometry.size.h),
        }
    }

    pub(crate) fn origin(
        self,
        geometry: Rect<i32, Logical>,
        drawn_at: Point<i32, Logical>,
        area: Point<i32, Logical>,
    ) -> Point<f64, Logical> {
        let stretch = self.stretch(geometry);
        let placed = self.rect.loc - area.to_f64();
        Point::new(placed.x / stretch.x, placed.y / stretch.y) + (drawn_at - geometry.loc).to_f64()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Piece {
    Shape {
        shape: RoundedRect,
        fill: Color,
        stroke: Option<Stroke>,
    },
    Label {
        rect: SceneRect,
        text: Arc<str>,
        size: f32,
        colour: Color,
    },
}

impl Piece {
    pub(crate) fn filled(shape: RoundedRect, fill: Color) -> Self {
        Self::Shape {
            shape,
            fill,
            stroke: None,
        }
    }

    pub(crate) fn outlined(shape: RoundedRect, fill: Color, width: f32, colour: Color) -> Self {
        Self::Shape {
            shape,
            fill,
            stroke: Some(Stroke::new(width, colour)),
        }
    }

    pub(crate) fn label(rect: SceneRect, text: Arc<str>, size: f32, colour: Color) -> Self {
        Self::Label {
            rect,
            text,
            size,
            colour,
        }
    }

    pub(crate) fn node(&self, alpha: f32) -> Node {
        match self {
            Self::Shape {
                shape,
                fill,
                stroke,
            } => {
                let node = Node::rounded_rect(shape.rect, shape.radii).fill(faded(*fill, alpha));
                match stroke {
                    Some(stroke) => {
                        node.stroke(Stroke::new(stroke.width, faded(stroke.color, alpha)))
                    }
                    None => node,
                }
            }
            Self::Label {
                rect,
                text,
                size,
                colour,
            } => Node::text(
                *rect,
                TextSpec::new(text.clone(), *size)
                    .align(Align::Center)
                    .wrap(Wrap::None)
                    .layout(Some(rect.width())),
            )
            .fill(faded(*colour, alpha)),
        }
    }
}

fn faded(colour: Color, alpha: f32) -> Color {
    colour.with_alpha(colour.a * alpha)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Panel {
    pub(crate) pieces: Vec<Piece>,
    pub(crate) alpha: f32,
}

impl Panel {
    pub(crate) fn new(pieces: Vec<Piece>, alpha: f32) -> Option<Self> {
        (!pieces.is_empty() && alpha > 0.0).then_some(Self { pieces, alpha })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Frosted {
    pub(crate) panes: Arc<Frost>,
    pub(crate) alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Moved {
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) shown: Visual,
}

impl Moved {
    pub(crate) fn placement(
        self,
        drawn_at: Point<i32, Logical>,
        area: Point<i32, Logical>,
        scale: f64,
    ) -> Placement {
        let stretch = self.shown.stretch(self.geometry);
        Placement {
            origin: self.shown.origin(self.geometry, drawn_at, area),
            scale: Scale {
                x: scale * stretch.x,
                y: scale * stretch.y,
            },
            alpha: self.shown.alpha,
            clip: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Leaving {
    pub(crate) snapshot: SnapshotId,
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) shown: Visual,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Portrayed {
    pub(crate) view: ViewId,
    pub(crate) root: SurfaceId,
    pub(crate) drawn_at: Point<i32, Logical>,
    pub(crate) moved: Moved,
    pub(crate) clip: Option<RoundedRect>,
    pub(crate) shape: Option<WindowShape>,
    pub(crate) caption: Option<CaptionPaint>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Staging {
    pub(crate) moved: HashMap<Target, Moved>,
    pub(crate) hidden: Vec<Target>,
    pub(crate) exiting: HashMap<SnapshotId, Visual>,
    pub(crate) leaving: Vec<Leaving>,
    pub(crate) beneath: Option<(WindowId, Panel)>,
    pub(crate) frosted: Option<Frosted>,
    pub(crate) over: Option<Panel>,
    pub(crate) lifted: Vec<Target>,
    pub(crate) top: Option<Panel>,
    pub(crate) portrayed: HashMap<SurfaceId, Vec<Portrayed>>,
    pub(crate) scrims: HashMap<Target, f32>,
    pub(crate) outlines: Vec<Outline>,
    pub(crate) shapes: HashMap<WindowId, WindowShape>,
    pub(crate) captions: HashMap<WindowId, CaptionPaint>,
}
