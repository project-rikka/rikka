use glam::Mat4;
use rikka_physics_tailor::{BodyShape, ClothAsset};

use crate::inputs::Joint;
use crate::layout::{ColliderRecord, Topology};

const OPPOSED: f32 = -0.95;

#[derive(Clone, Debug)]
pub(crate) struct Fitted {
    slot: usize,
    rest: Mat4,
    shape: BodyShape,
    inradius: f32,
}

impl Fitted {
    pub(crate) fn fit(asset: &ClothAsset, topology: &Topology) -> Vec<Fitted> {
        asset
            .body
            .colliders
            .iter()
            .map(|collider| Fitted {
                slot: topology.slot_of[&collider.node] as usize,
                rest: asset.rest_worlds[collider.node.index()],
                inradius: inradius(&collider.shape),
                shape: collider.shape.clone(),
            })
            .collect()
    }
}

pub(crate) fn plane_count(fitted: &[Fitted]) -> usize {
    fitted
        .iter()
        .map(|collider| match &collider.shape {
            BodyShape::Tapered { .. } => 0,
            BodyShape::Hull { planes } => planes.len(),
        })
        .sum()
}

pub(crate) fn place(
    fitted: &[Fitted],
    joints: &[Joint],
    colliders: &mut Vec<ColliderRecord>,
    planes: &mut Vec<[f32; 4]>,
) {
    place_with(fitted, |slot| joints[slot].matrix(), colliders, planes);
}

pub(crate) fn place_palettes(
    fitted: &[Fitted],
    palettes: &[Mat4],
    colliders: &mut Vec<ColliderRecord>,
    planes: &mut Vec<[f32; 4]>,
) {
    place_with(fitted, |slot| palettes[slot], colliders, planes);
}

fn place_with(
    fitted: &[Fitted],
    matrix_of: impl Fn(usize) -> Mat4,
    colliders: &mut Vec<ColliderRecord>,
    planes: &mut Vec<[f32; 4]>,
) {
    let mut offset = 0u32;
    for collider in fitted {
        let world = matrix_of(collider.slot) * collider.rest;
        match collider.shape.placed(world) {
            BodyShape::Tapered { ends, radii } => colliders.push(ColliderRecord {
                start: ends[0].extend(radii[0]).to_array(),
                end: ends[1].extend(radii[1]).to_array(),
                shape: [0, 0, 0, 0],
            }),
            BodyShape::Hull { planes: placed } => {
                colliders.push(ColliderRecord {
                    start: [0.0; 4],
                    end: [0.0; 4],
                    shape: [1, offset, placed.len() as u32, collider.inradius.to_bits()],
                });
                offset += placed.len() as u32;
                planes.extend(
                    placed
                        .iter()
                        .map(|(normal, offset)| normal.extend(*offset).to_array()),
                );
            }
        }
    }
}

fn inradius(shape: &BodyShape) -> f32 {
    match shape {
        BodyShape::Tapered { radii, .. } => radii[0].min(radii[1]),
        BodyShape::Hull { planes } => planes
            .iter()
            .filter_map(|(normal, offset)| {
                planes
                    .iter()
                    .filter(|(other, _)| normal.dot(*other) < OPPOSED)
                    .map(|(_, across)| 0.5 * (offset + across))
                    .reduce(f32::min)
            })
            .reduce(f32::min)
            .unwrap_or(0.0)
            .max(0.0),
    }
}
