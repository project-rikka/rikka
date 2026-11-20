use rikka_renderer::{Audience, Node, NodeId, Scene, Viewer};
use rikka_theme::Corner;
use rikka_types::{Color, CornerRadii, Rect, RoundedRect};

#[derive(Debug, Clone, Copy, PartialEq)]
struct Shape {
    width: i32,
    height: i32,
    radius: i32,
}

#[derive(Debug)]
pub(super) struct Corners {
    nodes: [NodeId; 4],
    shape: Shape,
}

impl Corners {
    pub(super) fn show(
        current: Option<Self>,
        scene: &mut Scene,
        size: (i32, i32),
        scale: f64,
        z: i32,
    ) -> Option<Self> {
        let radius = (f64::from(Corner::Window.radius()) * scale).round() as i32;
        let shape = Shape {
            width: size.0,
            height: size.1,
            radius,
        };
        if radius <= 0 || shape.width < radius * 2 || shape.height < radius * 2 {
            Self::hide(current, scene);
            return None;
        }
        match current {
            Some(current) if current.shape == shape => Some(current),
            current => {
                Self::hide(current, scene);
                let screen = RoundedRect::new(
                    Rect::new(0.0, 0.0, shape.width as f32, shape.height as f32),
                    CornerRadii::uniform(radius as f32),
                );
                let nodes = squares(shape).map(|square| {
                    scene.insert(
                        Node::rect(square)
                            .fill(Color::BLACK)
                            .cut_out(screen)
                            .z(z)
                            .audience(Audience::Only(Viewer::Screen)),
                    )
                });
                Some(Self { nodes, shape })
            }
        }
    }

    pub(super) fn hide(current: Option<Self>, scene: &mut Scene) {
        for node in current.into_iter().flat_map(|current| current.nodes) {
            scene.remove(node);
        }
    }
}

fn squares(shape: Shape) -> [Rect; 4] {
    let reach = (shape.radius as f32 * RoundedRect::CORNER_REACH).ceil() as i32;
    let side = reach.min(shape.width / 2).min(shape.height / 2);
    let (right, bottom) = ((shape.width - side) as f32, (shape.height - side) as f32);
    [(0.0, 0.0), (right, 0.0), (right, bottom), (0.0, bottom)]
        .map(|(x, y)| Rect::new(x, y, side as f32, side as f32))
}
