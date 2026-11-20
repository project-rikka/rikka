use rikka_compositor_geometry::{Logical, Physical, Point, Rect};
use rikka_renderer::Stroke;
use rikka_theme::Corner;
use rikka_types::{Color, Rect as SceneRect, RoundedRect};

use super::edge::Edge;
use super::tree::{Placement, physical};

const RADIUS: f32 = Corner::Window.radius();

const EDGE_WIDTH: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowShape {
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) rounding: f32,
}

impl WindowShape {
    pub(crate) fn at_rest(geometry: Rect<i32, Logical>, rounded: bool) -> Self {
        Self {
            geometry,
            rounding: if rounded { 1.0 } else { 0.0 },
        }
    }

    fn rounding(self) -> f32 {
        self.rounding.clamp(0.0, 1.0)
    }

    pub(crate) fn radius(self) -> f32 {
        RADIUS * self.rounding()
    }

    pub(crate) fn place(self, placement: &Placement, drawn_at: Point<i32, Logical>) -> Shaped {
        let inset = (self.geometry.loc - drawn_at).to_f64();
        let rect = physical(
            placement.origin + inset,
            self.geometry.size.to_f64(),
            placement.scale,
        );
        let zoom = placement.scale.x.min(placement.scale.y) as f32;
        Shaped::new(self, rect, zoom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shaped {
    pub(crate) shape: WindowShape,
    pub(crate) area: RoundedRect,
    zoom: f32,
}

impl Shaped {
    pub(crate) fn new(shape: WindowShape, rect: Rect<i32, Physical>, zoom: f32) -> Self {
        Self {
            shape,
            area: RoundedRect::new(SceneRect::from(rect), shape.radius() * zoom),
            zoom,
        }
    }

    pub(crate) fn clip(&self) -> Option<RoundedRect> {
        (self.area.radii.max() > 0.0).then_some(self.area)
    }

    pub(crate) fn edge(&self, colour: Color, alpha: f32) -> Option<Edge> {
        let shown = colour.scale_alpha(self.shape.rounding() * alpha);
        (shown.a > 0.0).then(|| Edge {
            shape: self.area,
            stroke: Stroke::new((EDGE_WIDTH * self.zoom).round().max(1.0), shown),
            clip: None,
        })
    }
}
