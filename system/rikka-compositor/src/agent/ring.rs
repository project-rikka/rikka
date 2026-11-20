use std::collections::HashMap;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_output::CaptureSessionId;
use rikka_compositor_types::OutputId;
use rikka_renderer::{Node, NodeId, Scene, Stroke};
use rikka_types::{Color, Point as ScenePoint};

use crate::outputs::Output;

const SIDE: f64 = 22.0;

const EDGE: f32 = 2.0;

const FILLED: f32 = 0.22;

const RING_Z: i32 = i32::MAX - 2048;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Circle {
    centre: ScenePoint,
    radius: f32,
    edge: f32,
    colour: Color,
}

impl Circle {
    fn new(local: Point<f64, Logical>, scale: f64, colour: Color) -> Option<Self> {
        let side = (SIDE * scale).round() as f32;
        if side <= 0.0 {
            return None;
        }
        let edge = (EDGE * side / SIDE as f32).max(1.0);
        Some(Self {
            centre: ScenePoint::new(
                (local.x * scale).round() as f32,
                (local.y * scale).round() as f32,
            ),
            radius: (side / 2.0 - edge).max(1.0),
            edge,
            colour,
        })
    }

    fn node(self) -> Node {
        Node::circle(self.centre, self.radius)
            .fill(self.colour.with_alpha(FILLED))
            .stroke(Stroke::new(self.edge, self.colour))
            .z(RING_Z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Canvas {
    Output(OutputId),
    Capture(CaptureSessionId),
}

#[derive(Debug, Default)]
pub(crate) struct Ring {
    drawn: HashMap<Canvas, (NodeId, Circle)>,
}

impl Ring {
    pub(crate) fn draw(
        &mut self,
        scene: &mut Scene,
        canvas: Canvas,
        output: &Output,
        at: Option<Point<f64, Logical>>,
        colour: Color,
    ) {
        let next = at.and_then(|at| {
            let area = output.area()?.to_f64();
            area.contains(at)
                .then(|| Circle::new(at - area.loc, output.scale(), colour))
                .flatten()
        });
        let current = self
            .drawn
            .remove(&canvas)
            .filter(|(node, _)| scene.get(*node).is_some());
        match (current, next) {
            (Some((node, circle)), Some(next)) if circle == next => {
                self.drawn.insert(canvas, (node, circle));
            }
            (current, next) => {
                if let Some((node, _)) = current {
                    scene.remove(node);
                }
                if let Some(circle) = next {
                    let node = scene.insert(circle.node());
                    self.drawn.insert(canvas, (node, circle));
                }
            }
        }
    }

    pub(crate) fn forget(&mut self, canvas: Canvas) {
        self.drawn.remove(&canvas);
    }
}
