use rikka_compositor_geometry::{Physical, Rect};
use rikka_renderer::{NodeId, Scene};

use super::depth::Depth;
use super::staging::{Panel, Piece};

#[derive(Debug)]
struct PanelState {
    panel: Panel,
    nodes: Vec<NodeId>,
    z: i32,
}

#[derive(Debug, Default)]
pub(crate) struct PanelNodes(Option<PanelState>);

impl PanelNodes {
    pub(crate) fn show(&mut self, scene: &mut Scene, panel: Option<&Panel>, depth: &mut Depth) {
        let Some(panel) = panel else {
            return self.hide(scene);
        };
        let base = depth.reserve(panel.pieces.len());
        if self
            .0
            .as_ref()
            .is_some_and(|shown| shown.panel == *panel && shown.z == base)
        {
            return;
        }
        self.hide(scene);
        let nodes = panel
            .pieces
            .iter()
            .zip(base + 1..)
            .map(|(piece, z)| scene.insert(piece.node(panel.alpha).z(z)))
            .collect();
        self.0 = Some(PanelState {
            panel: panel.clone(),
            nodes,
            z: base,
        });
    }

    pub(crate) fn hide(&mut self, scene: &mut Scene) {
        if let Some(shown) = self.0.take() {
            for node in shown.nodes {
                scene.remove(node);
            }
        }
    }

    pub(crate) fn drawn(&self) -> impl Iterator<Item = (usize, Rect<i32, Physical>, f32)> + '_ {
        self.0.iter().flat_map(|shown| {
            let alpha = shown.panel.alpha;
            shown
                .panel
                .pieces
                .iter()
                .enumerate()
                .map(move |(index, piece)| {
                    let rect = match piece {
                        Piece::Shape { shape, stroke, .. } => {
                            let expand = stroke.map_or(0.0, |stroke| stroke.width);
                            shape.rect.inflate(expand)
                        }
                        Piece::Label { rect, .. } => *rect,
                    };
                    (index, Rect::covering(rect), alpha)
                })
        })
    }
}
