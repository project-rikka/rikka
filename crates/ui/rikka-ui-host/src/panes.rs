use rikka_renderer::{Blur, Node, NodeId, Scene};
use rikka_theme::Translucency;
use rikka_types::{Color, Lens, RoundedRect};

const PANE_Z: i32 = -1;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Frost {
    radius: f32,
    lens: Option<Lens>,
}

#[derive(Debug, Default)]
pub(crate) struct Panes {
    shapes: Vec<RoundedRect>,
    scale: f32,
    frost: Option<Frost>,
    nodes: Vec<NodeId>,
}

impl Panes {
    pub(crate) fn follow(
        &mut self,
        scene: &mut Scene,
        shapes: &[RoundedRect],
        translucency: Translucency,
        scale: f32,
    ) {
        let lens = translucency.lens();
        let frost = translucency.blur_radius().map(|radius| Frost {
            radius: radius * lens.map_or(1.0, Lens::frost) * scale,
            lens: lens.map(|lens| lens.scaled(scale)),
        });
        if self.frost == frost && self.scale == scale && self.shapes == shapes {
            return;
        }
        for node in self.nodes.drain(..) {
            scene.remove(node);
        }
        self.shapes.clear();
        self.shapes.extend_from_slice(shapes);
        self.scale = scale;
        self.frost = frost;
        let Some(frost) = frost else {
            return;
        };
        self.nodes.extend(shapes.iter().map(|shape| {
            let placed = shape.scale(scale);
            let pane = Node::rounded_rect(placed.rect, placed.radii)
                .fill(Color::TRANSPARENT)
                .backdrop(Blur::with_radius(frost.radius))
                .z(PANE_Z);
            scene.insert(match frost.lens {
                Some(lens) => pane.lens(lens),
                None => pane,
            })
        }));
    }
}
