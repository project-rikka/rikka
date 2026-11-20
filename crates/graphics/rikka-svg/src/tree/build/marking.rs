use std::sync::Arc;

use rikka_types::Rect;
use rikka_vector::{Path, Transform};

use crate::parse::dom::{NodeId, Tag};
use crate::parse::values::{aspect_ratio, fit, reference, view_box};
use crate::tree::build::{Builder, ContextPaint};
use crate::tree::levels::Levels;
use crate::tree::markers::{Orientation, Position, orientation, vertices};
use crate::tree::style::{Axis, Context};
use crate::tree::{Clip, Fill, Group, Node, Outline};

const DEFAULT_SIDE: f32 = 3.0;
const MARKERS: [(&str, Position); 3] = [
    ("marker-start", Position::Start),
    ("marker-mid", Position::Middle),
    ("marker-end", Position::End),
];

struct Marker {
    placement: Transform,
    scale: (f32, f32),
    window: Option<Rect>,
    orientation: Orientation,
    content: Vec<Node>,
    cost: usize,
}

impl Builder<'_> {
    pub(super) fn markers(
        &mut self,
        path: &Path,
        fill: Option<&Fill>,
        stroke: Option<&Outline>,
    ) -> Vec<Node> {
        let context = self.context();
        let stroke_width = context
            .length("stroke-width", Axis::Diagonal)
            .filter(|width| *width > 0.0)
            .unwrap_or(1.0);
        let referenced: Vec<(NodeId, Position)> = MARKERS
            .iter()
            .filter_map(|(name, position)| {
                let value = context.property(name)?;
                let node = reference(&value).and_then(|id| self.dom.find(id, &[Tag::Marker]))?;
                Some((node, *position))
            })
            .collect();
        let paint = ContextPaint {
            fill: fill.map(|fill| fill.ink.clone()),
            stroke: stroke.map(|stroke| stroke.ink.clone()),
        };
        let mut out = Vec::new();
        for (node, position) in referenced {
            let Some(marker) = self.marker(node, stroke_width, &paint) else {
                continue;
            };
            for vertex in vertices(path, position, marker.orientation) {
                if !self.spend(marker.cost) {
                    self.exhausted = true;
                    return out;
                }
                let placement = marker
                    .placement
                    .then(Transform::scale(marker.scale.0, marker.scale.1))
                    .then(Transform::rotate(vertex.angle))
                    .then(Transform::translate(vertex.point.x, vertex.point.y));
                out.push(Node::Group(Group {
                    transform: placement,
                    clip: marker.window.map(|window| {
                        let mut path = Path::new();
                        path.add_rect(window);
                        Arc::new(Clip::window(path))
                    }),
                    children: marker.content.clone(),
                    ..Group::default()
                }));
            }
        }
        out
    }

    fn marker(&mut self, node: NodeId, stroke_width: f32, paint: &ContextPaint) -> Option<Marker> {
        let start = self.levels;
        let nested = Levels {
            nesting: start.nesting + 1,
            ..start
        };
        if self.building.contains(&node) || !nested.fits() {
            return None;
        }
        let ancestry = self.ancestry(node);
        let context = Context {
            dom: self.dom,
            chain: &ancestry,
            viewport: self.viewport,
        };
        let side = |name: &str, axis: Axis| context.coordinate(name, axis).unwrap_or(DEFAULT_SIDE);
        let (width, height) = (
            side("markerWidth", Axis::Horizontal),
            side("markerHeight", Axis::Vertical),
        );
        if !(width > 0.0 && height > 0.0) {
            return None;
        }
        let reference_point = (
            context.coordinate("refX", Axis::Horizontal).unwrap_or(0.0),
            context.coordinate("refY", Axis::Vertical).unwrap_or(0.0),
        );
        let units = match context.attribute("markerUnits") {
            Some("userSpaceOnUse") => 1.0,
            _ => stroke_width,
        };
        let view = context.attribute("viewBox").and_then(view_box);
        let scale = match view {
            Some(view) => {
                let aspect = context
                    .attribute("preserveAspectRatio")
                    .map(aspect_ratio)
                    .unwrap_or_default();
                let fitted = fit(view, aspect, width * units, height * units);
                (fitted.xx.hypot(fitted.yx), fitted.xy.hypot(fitted.yy))
            }
            None => (units, units),
        };
        let clipped = !matches!(
            context.property("overflow").as_deref(),
            Some("visible" | "auto")
        );
        let window = clipped.then(|| match view {
            Some(view) => Rect::new(view.x, view.y, view.width, view.height),
            None => Rect::new(0.0, 0.0, width, height),
        });
        let orientation = orientation(context.attribute("orient"));
        let before = self.sources.budget.remaining();
        let chain = std::mem::replace(&mut self.chain, ancestry);
        let outer_paint = self.context_paint.replace(paint.clone());
        self.building.push(node);
        self.enter(nested);
        let mut content = Vec::new();
        self.within_layers(usize::from(clipped), |builder| {
            builder.children(node, &mut content);
        });
        self.levels = start;
        self.building.pop();
        self.context_paint = outer_paint;
        self.chain = chain;
        let cost = before
            .saturating_sub(self.sources.budget.remaining())
            .max(size_of::<Node>());
        (!content.is_empty()).then_some(Marker {
            placement: Transform::translate(-reference_point.0, -reference_point.1),
            scale,
            window,
            orientation,
            content,
            cost,
        })
    }
}
