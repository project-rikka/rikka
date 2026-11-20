use rikka_compositor_geometry::{Physical, Rect};
use rikka_renderer::{Audience, Node, NodeId, Scene, Stroke};
use rikka_types::{Color, RoundedRect};

use super::depth::Depth;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Edge {
    pub(crate) shape: RoundedRect,
    pub(crate) stroke: Stroke,
    pub(crate) clip: Option<RoundedRect>,
}

#[derive(Debug)]
struct Drawn {
    node: NodeId,
    edge: Edge,
    z: i32,
    audience: Audience,
}

#[derive(Debug, Default)]
pub(crate) struct EdgeNode {
    drawn: Option<Drawn>,
}

impl EdgeNode {
    pub(crate) fn draw(
        &mut self,
        scene: &mut Scene,
        edge: Option<Edge>,
        audience: Audience,
        depth: &mut Depth,
    ) {
        let Some(edge) = edge else {
            self.clear(scene);
            return;
        };
        let z = depth.next();
        match &mut self.drawn {
            Some(drawn) if (drawn.edge, drawn.z, drawn.audience) == (edge, z, audience) => {}
            Some(drawn) => {
                scene
                    .edit(drawn.node)
                    .rect(edge.shape.rect)
                    .radii(edge.shape.radii)
                    .stroke(edge.stroke)
                    .clip(edge.clip)
                    .z(z)
                    .audience(audience);
                drawn.edge = edge;
                drawn.z = z;
                drawn.audience = audience;
            }
            None => {
                let node = Node::rounded_rect(edge.shape.rect, edge.shape.radii)
                    .fill(Color::TRANSPARENT)
                    .stroke(edge.stroke)
                    .z(z)
                    .audience(audience);
                let node = scene.insert(match edge.clip {
                    Some(clip) => node.clip(clip),
                    None => node,
                });
                self.drawn = Some(Drawn {
                    node,
                    edge,
                    z,
                    audience,
                });
            }
        }
    }

    pub(crate) fn clear(&mut self, scene: &mut Scene) {
        if let Some(drawn) = self.drawn.take() {
            scene.remove(drawn.node);
        }
    }

    pub(crate) fn bounds(&self) -> Option<(Rect<i32, Physical>, f32, Option<RoundedRect>)> {
        let drawn = self.drawn.as_ref()?;
        let rect = Rect::covering(drawn.edge.shape.rect.inflate(drawn.edge.stroke.width));
        Some((rect, drawn.edge.stroke.color.a, drawn.edge.clip))
    }
}
