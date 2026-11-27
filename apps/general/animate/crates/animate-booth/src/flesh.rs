use rikka_model::glam::Vec3;
use rikka_model::{Bounds, Instance, Model, NodeId};

#[derive(Clone, Copy, Debug)]
struct Piece {
    joint: NodeId,
    center: Vec3,
    radius: f32,
}

#[derive(Default)]
struct Gathered {
    sum: Vec3,
    count: u32,
    points: Vec<Vec3>,
}

#[derive(Debug, Default)]
pub(crate) struct Flesh {
    pieces: Vec<Piece>,
}

fn strongest(joints: &[u16; 4], weights: [f32; 4]) -> Option<u16> {
    joints
        .iter()
        .zip(weights)
        .filter(|(_, weight)| *weight > 0.0)
        .max_by(|one, two| one.1.total_cmp(&two.1))
        .map(|(joint, _)| *joint)
}

impl Flesh {
    pub(crate) fn new(model: &Model) -> Self {
        let mut gathered: Vec<Gathered> = Vec::new();
        gathered.resize_with(model.nodes.len(), Gathered::default);
        for node in &model.nodes {
            let (Some(mesh), Some(skin)) = (node.mesh, node.skin) else {
                continue;
            };
            let vertices = &model.meshes[mesh.index()].vertices;
            let skin = &model.skins[skin.index()];
            let influences = vertices.joints.iter().zip(&vertices.weights);
            for (position, (joints, weights)) in vertices.positions.iter().zip(influences) {
                let Some(slot) = strongest(joints, weights.to_array()).map(usize::from) else {
                    continue;
                };
                let (Some(joint), Some(inverse_bind)) =
                    (skin.joints.get(slot), skin.inverse_bind.get(slot))
                else {
                    continue;
                };
                let local = inverse_bind.transform_point3(*position);
                let gathering = &mut gathered[joint.index()];
                gathering.sum += local;
                gathering.count += 1;
                gathering.points.push(local);
            }
        }
        let pieces = gathered
            .into_iter()
            .enumerate()
            .filter(|(_, gathering)| gathering.count > 0)
            .map(|(index, gathering)| {
                let center = gathering.sum / gathering.count as f32;
                let radius = gathering
                    .points
                    .iter()
                    .map(|point| point.distance(center))
                    .fold(0.0, f32::max);
                Piece {
                    joint: NodeId(index as u32),
                    center,
                    radius,
                }
            })
            .collect();
        Self { pieces }
    }

    pub(crate) fn bounds(&self, instance: &Instance) -> Option<Bounds> {
        self.pieces
            .iter()
            .map(|piece| {
                let center = instance.world(piece.joint).transform_point3(piece.center);
                Bounds {
                    min: center - piece.radius,
                    max: center + piece.radius,
                }
            })
            .reduce(Bounds::union)
    }
}
