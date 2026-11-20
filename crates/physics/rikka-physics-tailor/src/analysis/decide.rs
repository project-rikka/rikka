use rikka_model_core::Bounds;

use super::class::Class;
use super::features::{FREE_SHARE, MeshFeatures};
use crate::cloth_settings::Role;

const SCALP: f32 = 0.012;
const ROOT_BAND: f32 = 0.015;
const TIGHT: f32 = 0.008;
const LOOSE_SHARE: f32 = 0.2;
const LOOSE_EXTENT: f32 = 0.05;
const CORE_SHARE: f32 = 0.5;
const TOP_RING: f32 = 0.015;
const NEAREST_RING: f32 = 0.01;
const CROP: f32 = 0.04;
const CROP_BODY: f32 = 0.06;
const SOLID_COUNT: usize = 24;
const SOLID_EXTENT: f32 = 0.03;
const HAIR_DOMINANCE: f32 = 0.8;
const HAIR_EXTENT: f32 = 0.05;

#[derive(Clone, Debug)]
pub(crate) struct Auto {
    pub(crate) roles: Vec<Role>,
    pub(crate) strands: Vec<bool>,
}

pub(crate) fn decide(features: &mut MeshFeatures<'_, '_>) -> Auto {
    let count = features.figure.vertices.len();
    let mut auto = Auto {
        roles: vec![Role::Skinned; count],
        strands: vec![false; count],
    };
    let hair_exists = features.hair_exists();
    let hair_hinted = features.hair_hinted();
    let garments_hinted = features.garments_hinted();
    let head_top = features.head.map(|_| {
        features
            .figure
            .vertices
            .iter()
            .filter(|vertex| features.figure.skeleton.under_head[vertex.strongest().index()])
            .map(|vertex| vertex.position.y)
            .fold(f32::MIN, f32::max)
    });
    for index in 0..features.regions.len() {
        let region = features.regions[index].clone();
        match region.class {
            Class::Detail => {}
            Class::Skin => assign(&mut auto, &region.vertices, |_| Role::Body),
            Class::Solid => {
                if region.hinted {
                    hinted(features, &mut auto, &region.vertices, false)
                } else {
                    rest(&mut auto, &region.vertices, &region.bounds)
                }
            }
            Class::Hair => {
                if !(hair_hinted && !region.hinted) {
                    hair(features, &mut auto, &region.vertices, region.hinted)
                }
            }
            Class::Garment | Class::Unknown => {
                let geometric_hair = !hair_exists
                    && head_top.is_some_and(|top| {
                        looks_like_hair(features, &region.vertices, &region.bounds, top)
                    });
                if region.hinted {
                    hinted(features, &mut auto, &region.vertices, geometric_hair);
                } else if geometric_hair {
                    hair(features, &mut auto, &region.vertices, false);
                } else if garments_hinted || !loose(features, &mut auto, &region.vertices) {
                    rest(&mut auto, &region.vertices, &region.bounds);
                }
            }
        }
    }
    crop(features, &mut auto);
    auto
}

fn assign(auto: &mut Auto, vertices: &[u32], role: impl Fn(u32) -> Role) {
    for vertex in vertices {
        auto.roles[*vertex as usize] = role(*vertex);
    }
}

fn rest(auto: &mut Auto, vertices: &[u32], bounds: &Bounds) {
    let solid = vertices.len() >= SOLID_COUNT && bounds.longest() >= SOLID_EXTENT;
    let role = if solid { Role::Body } else { Role::Skinned };
    assign(auto, vertices, |_| role);
}

fn hinted(features: &MeshFeatures<'_, '_>, auto: &mut Auto, vertices: &[u32], strands: bool) {
    let pins: Vec<bool> = vertices
        .iter()
        .map(|vertex| features.share[*vertex as usize] < FREE_SHARE)
        .collect();
    simulate(features, auto, vertices, pins, strands);
}

fn hair(
    features: &mut MeshFeatures<'_, '_>,
    auto: &mut Auto,
    vertices: &[u32],
    hinted_region: bool,
) {
    if hinted_region {
        hinted(features, auto, vertices, true);
        return;
    }
    let radial: Vec<f32> = match features.head {
        Some(head) => vertices
            .iter()
            .map(|vertex| features.position(*vertex).distance(head.centre))
            .collect(),
        None => vec![f32::MAX; vertices.len()],
    };
    let nearest = radial.iter().copied().fold(f32::MAX, f32::min);
    let pins: Vec<bool> = vertices
        .iter()
        .zip(&radial)
        .map(|(vertex, radial)| features.offset(*vertex) <= SCALP || *radial <= nearest + ROOT_BAND)
        .collect();
    simulate(features, auto, vertices, pins, true);
}

fn loose(features: &mut MeshFeatures<'_, '_>, auto: &mut Auto, vertices: &[u32]) -> bool {
    let tight: Vec<bool> = vertices
        .iter()
        .map(|vertex| features.offset(*vertex) <= TIGHT)
        .collect();
    let free: Vec<u32> = vertices
        .iter()
        .zip(&tight)
        .filter(|(_, tight)| !**tight)
        .map(|(vertex, _)| *vertex)
        .collect();
    if free.is_empty() {
        return false;
    }
    let extent =
        Bounds::from_points(free.iter().map(|vertex| features.position(*vertex))).longest();
    let core = free
        .iter()
        .filter(|vertex| {
            let strongest = features.figure.vertices[**vertex as usize].strongest();
            features.figure.skeleton.core[strongest.index()]
        })
        .count();
    let geometric = free.len() as f32 >= LOOSE_SHARE * vertices.len() as f32
        && extent >= LOOSE_EXTENT
        && core as f32 >= CORE_SHARE * free.len() as f32
        && free.len() < vertices.len();
    if !geometric {
        return false;
    }
    simulate(features, auto, vertices, tight, false);
    true
}

fn simulate(
    features: &MeshFeatures<'_, '_>,
    auto: &mut Auto,
    vertices: &[u32],
    mut pins: Vec<bool>,
    strands: bool,
) {
    if pins.iter().all(|pinned| *pinned) {
        return;
    }
    if strands && !pins.iter().any(|pinned| *pinned) {
        pins = strand_roots(features, vertices);
    }
    for (vertex, pinned) in vertices.iter().zip(pins) {
        auto.roles[*vertex as usize] = if pinned { Role::Pinned } else { Role::Free };
        auto.strands[*vertex as usize] = strands;
    }
}

fn strand_roots(features: &MeshFeatures<'_, '_>, vertices: &[u32]) -> Vec<bool> {
    match features.head {
        Some(head) => {
            let radial: Vec<f32> = vertices
                .iter()
                .map(|vertex| features.position(*vertex).distance(head.centre))
                .collect();
            let nearest = radial.iter().copied().fold(f32::MAX, f32::min);
            radial
                .iter()
                .map(|radial| *radial <= nearest + NEAREST_RING)
                .collect()
        }
        None => {
            let top = vertices
                .iter()
                .map(|vertex| features.position(*vertex).y)
                .fold(f32::MIN, f32::max);
            vertices
                .iter()
                .map(|vertex| features.position(*vertex).y >= top - TOP_RING)
                .collect()
        }
    }
}

fn looks_like_hair(
    features: &MeshFeatures<'_, '_>,
    vertices: &[u32],
    bounds: &Bounds,
    head_top: f32,
) -> bool {
    let Some(head) = features.head else {
        return false;
    };
    let under = &features.figure.skeleton.under_head;
    let dominated = vertices
        .iter()
        .filter(|vertex| {
            under[features.figure.vertices[**vertex as usize]
                .strongest()
                .index()]
        })
        .count();
    let neck = features.figure.skeleton.origins[head.node.index()].y;
    dominated as f32 >= HAIR_DOMINANCE * vertices.len() as f32
        && bounds.longest() >= HAIR_EXTENT
        && (bounds.max.y >= head_top - TOP_RING || bounds.min.y < neck - LOOSE_EXTENT)
}

fn crop(features: &mut MeshFeatures<'_, '_>, auto: &mut Auto) {
    let free = auto
        .roles
        .iter()
        .enumerate()
        .filter(|(_, role)| **role == Role::Free)
        .map(|(vertex, _)| vertex as u32);
    let links =
        features.links(|vertex| matches!(auto.roles[vertex as usize], Role::Free | Role::Pinned));
    let reached = links.reach(free, CROP_BODY);
    for vertex in 0..auto.roles.len() {
        if auto.roles[vertex] != Role::Pinned {
            continue;
        }
        let distance = reached.distance[vertex];
        if distance <= CROP {
            continue;
        }
        let garment = !auto.strands[vertex];
        auto.roles[vertex] =
            if garment && distance > CROP_BODY && features.offset(vertex as u32) <= TIGHT {
                Role::Body
            } else {
                Role::Skinned
            };
    }
}
