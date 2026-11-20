use std::collections::HashMap;

use glam::{Quat, Vec3};
use rikka_model_core::{Model, NodeId};
use rikka_physics_tailor::{BindTarget, ClothAsset, Piece, PieceKind};

use crate::support::{avatar, avatar_asset, miku, miku_asset};

const RADIUS: f32 = 3.0;
const AXIS_TOLERANCE: f32 = 0.03;
const EDGE_TOLERANCE: f32 = 0.1;
const SLACK: f32 = 0.002;

struct Bend {
    top: f32,
    across: f32,
    turn: Quat,
    shift: Vec3,
}

impl Bend {
    fn from_positions(positions: &[Vec3]) -> Bend {
        Bend {
            top: positions.iter().map(|at| at.y).fold(f32::MIN, f32::max),
            across: positions.iter().map(|at| at.z).sum::<f32>() / positions.len() as f32,
            turn: Quat::from_euler(glam::EulerRot::YXZ, 0.7, -0.4, 0.3),
            shift: Vec3::new(0.3, -0.2, 0.1),
        }
    }

    fn apply(&self, at: Vec3) -> Vec3 {
        let angle = (self.top - at.y) / RADIUS;
        let from_centre = RADIUS - (at.z - self.across);
        let bent = Vec3::new(
            at.x,
            self.top - from_centre * angle.sin(),
            self.across + RADIUS - from_centre * angle.cos(),
        );
        self.turn * bent + self.shift
    }
}

fn render_edges(model: &Model, piece: &Piece) -> Vec<(usize, usize)> {
    let mut slot: HashMap<(NodeId, u32), usize> = HashMap::new();
    for (at, bind) in piece.bindings.iter().enumerate() {
        slot.insert((bind.node, bind.vertex), at);
    }
    let mut nodes: Vec<NodeId> = piece.bindings.iter().map(|bind| bind.node).collect();
    nodes.sort_unstable();
    nodes.dedup();
    let mut edges = Vec::new();
    for node in nodes {
        let Some(mesh) = model.node(node).mesh else {
            continue;
        };
        for corners in model.mesh(mesh).indices.as_chunks::<3>().0 {
            let inside: Vec<usize> = corners
                .iter()
                .filter_map(|corner| slot.get(&(node, *corner)).copied())
                .collect();
            if inside.len() == 3 {
                edges.extend([
                    (inside[0], inside[1]),
                    (inside[1], inside[2]),
                    (inside[2], inside[0]),
                ]);
            }
        }
    }
    edges
}

fn check(name: &str, model: &Model, asset: &ClothAsset) {
    for piece in asset
        .pieces
        .iter()
        .filter(|piece| piece.kind == PieceKind::Strands)
    {
        let rest = piece.rest_positions();
        let bend = Bend::from_positions(&rest);
        let bent: Vec<Vec3> = rest.iter().map(|at| bend.apply(*at)).collect();
        let before = piece.drape(&rest);
        let after = piece.drape(&bent);
        for ((bind, before), after) in piece.bindings.iter().zip(&before).zip(&after) {
            let BindTarget::Strand(strand) = &bind.target else {
                panic!("{name} {}: hair bound to triangles", piece.name);
            };
            let rest_gap = before
                .position
                .distance(strand.origin(&piece.strand_nodes, &rest));
            let bent_gap = after
                .position
                .distance(strand.origin(&piece.strand_nodes, &bent));
            assert!(
                (bent_gap - rest_gap).abs() <= AXIS_TOLERANCE * rest_gap + 1e-4,
                "{name} {}: vertex {} moves from {rest_gap} to {bent_gap} m off its axis",
                piece.name,
                bind.vertex
            );
        }
        for (a, b) in render_edges(model, piece) {
            let expected = bend
                .apply(before[a].position)
                .distance(bend.apply(before[b].position));
            let got = after[a].position.distance(after[b].position);
            assert!(
                (got - expected).abs() <= EDGE_TOLERANCE * expected + SLACK,
                "{name} {}: an edge of {expected} m tears to {got} m",
                piece.name
            );
        }
    }
}

#[test]
fn bent_clump_keeps_cross_section() {
    check("miku", miku(), miku_asset());
    check("avatar", avatar(), avatar_asset());
}
