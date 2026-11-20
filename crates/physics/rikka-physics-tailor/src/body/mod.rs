mod chain;
mod circle;
mod cone;
mod fit;
mod hull;
mod pose;
mod solid;

use glam::Vec3;
use rikka_humanoid::Anatomy;
use rikka_model_core::{Model, NodeId};

use crate::analysis::ClothAnalysis;
use crate::asset::{Body, BodyCollider, BodyVertex};
use crate::cloth_settings::Role;
use crate::error::Result;
use crate::figure::{Figure, Skeleton};
use crate::frame::particle_normals;
use crate::geometry::{Decimator, PointGrid, Shell, Union, is_proper};
use pose::Pose;
use solid::Solid;

const WELD: f32 = 1e-4;
const MAX_TRIANGLES: usize = 8_000;
const FEWEST: usize = 24;
const ENCLOSED: f32 = 0.001;
const MARGIN: f32 = 0.002;
const SHRINK_SHARE: f32 = 0.35;
const INTRUSION_QUANTILE_DENOM: usize = 10;

#[derive(Debug)]
pub(crate) struct Skin {
    pub(crate) body: Body,
    pub(crate) shell: Shell,
    pub(crate) normals: Vec<Vec3>,
}

pub(crate) fn skin(figure: &Figure<'_>, roles: &[Role]) -> Skin {
    let members: Vec<u32> = (0..roles.len() as u32)
        .filter(|vertex| roles[*vertex as usize] == Role::Body)
        .collect();
    let grid = PointGrid::new(
        members
            .iter()
            .enumerate()
            .map(|(at, vertex)| (at as u32, figure.vertices[*vertex as usize].position)),
        WELD * 4.0,
    );
    let mut union = Union::new(members.len());
    for (at, vertex) in members.iter().enumerate() {
        let position = figure.vertices[*vertex as usize].position;
        for other in grid.near(position, WELD) {
            if other as usize > at
                && figure.vertices[members[other as usize] as usize]
                    .position
                    .distance(position)
                    <= WELD
            {
                union.join(at as u32, other);
            }
        }
    }
    let (labels, count) = union.labels();
    let mut source = vec![u32::MAX; count];
    for (vertex, label) in members.iter().zip(&labels) {
        if source[*label as usize] == u32::MAX {
            source[*label as usize] = *vertex;
        }
    }
    let mut slot = vec![u32::MAX; figure.vertices.len()];
    for (vertex, label) in members.iter().zip(&labels) {
        slot[*vertex as usize] = *label;
    }
    let positions: Vec<Vec3> = source
        .iter()
        .map(|vertex| figure.vertices[*vertex as usize].position)
        .collect();
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut unique: Vec<([u32; 3], usize)> = Vec::new();
    for triangle in &figure.triangles {
        let mapped = triangle.corners.map(|corner| slot[corner as usize]);
        if mapped.contains(&u32::MAX) || !is_proper(mapped.map(|corner| positions[corner as usize]))
        {
            continue;
        }
        let mut key = mapped;
        key.sort_unstable();
        unique.push((key, triangles.len()));
        triangles.push(mapped);
    }
    unique.sort_unstable();
    unique.dedup_by(|later, earlier| later.0 == earlier.0);
    let mut indices: Vec<usize> = unique.into_iter().map(|(_, index)| index).collect();
    indices.sort_unstable();
    let triangles: Vec<[u32; 3]> = indices.into_iter().map(|index| triangles[index]).collect();
    let (positions, triangles, source) = if triangles.len() > MAX_TRIANGLES {
        decimated(positions, triangles, source)
    } else {
        (positions, triangles, source)
    };
    let vertices = positions
        .iter()
        .zip(&source)
        .map(|(position, vertex)| {
            let rest = &figure.vertices[*vertex as usize];
            BodyVertex {
                rest: *position,
                joints: rest.joints,
                weights: rest.weights,
            }
        })
        .collect();
    Skin {
        shell: Shell::new(&positions, &triangles),
        normals: particle_normals(&positions, &triangles),
        body: Body {
            vertices,
            triangles,
            colliders: Vec::new(),
        },
    }
}

fn decimated(
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
    source: Vec<u32>,
) -> (Vec<Vec3>, Vec<[u32; 3]>, Vec<u32>) {
    let count = positions.len();
    let target = count * MAX_TRIANGLES / triangles.len().max(1);
    let shortest = triangles
        .iter()
        .flat_map(|triangle| (0..3).map(move |side| (triangle[side], triangle[(side + 1) % 3])))
        .map(|(a, b)| positions[a as usize].distance(positions[b as usize]))
        .fold(f32::MAX, f32::min)
        .max(1e-4);
    let mut decimator = Decimator::new(positions, vec![false; count], triangles);
    decimator.reduce_to(shortest, target);
    let reduced = decimator.finish();
    let mut sources = vec![u32::MAX; reduced.positions.len()];
    for (old, new) in reduced.index.iter().enumerate() {
        if *new != u32::MAX && sources[*new as usize] == u32::MAX {
            sources[*new as usize] = source[old];
        }
    }
    (reduced.positions, reduced.triangles, sources)
}

impl Body {
    pub fn from_model(model: &Model, anatomy: &Anatomy, analysis: &ClothAnalysis) -> Result<Body> {
        let figure = Figure::new(model, anatomy)?;
        Ok(skin(&figure, analysis.roles()).body)
    }
}

pub(crate) fn colliders(
    figure: &Figure<'_>,
    roles: &[Role],
    samples: &[(Vec3, f32)],
) -> Vec<BodyCollider> {
    let skeleton = &figure.skeleton;
    let mut counts = vec![0usize; skeleton.parents.len()];
    for (vertex, role) in figure.vertices.iter().zip(roles) {
        if *role == Role::Body {
            counts[vertex.strongest().index()] += 1;
        }
    }
    let owners = owners(skeleton, counts);
    let mut points: Vec<Vec<Vec3>> = vec![Vec::new(); skeleton.parents.len()];
    for (vertex, role) in figure.vertices.iter().zip(roles) {
        if *role == Role::Body
            && let Some(owner) = owners[vertex.strongest().index()]
        {
            points[owner.index()].push(vertex.position);
        }
    }
    let clouds: Vec<(NodeId, Vec<Vec3>)> = skeleton
        .order
        .iter()
        .filter_map(|node| {
            let cloud = std::mem::take(&mut points[node.index()]);
            (!cloud.is_empty()).then_some((*node, cloud))
        })
        .collect();
    let mut colliders = Vec::new();
    for limb in limbs(clouds) {
        let local = Pose::from_matrix(figure.rest_worlds[limb.node.index()]).inverse();
        for solid in limb.solids {
            let solid = fitted(solid, samples);
            colliders.push(BodyCollider {
                node: limb.node,
                shape: solid.moved(local).shape(),
            });
        }
    }
    colliders
}

fn fitted(solid: Solid, samples: &[(Vec3, f32)]) -> Solid {
    let mut intrusions: Vec<f32> = samples
        .iter()
        .map(|(point, radius)| MARGIN - (solid.distance(*point) - radius))
        .filter(|depth| *depth > 0.0)
        .collect();
    if intrusions.is_empty() {
        return solid;
    }
    let index = intrusions.len() / INTRUSION_QUANTILE_DENOM;
    let (_, by, _) = intrusions.select_nth_unstable_by(index, |a, b| b.total_cmp(a));
    let by = by.min(solid.thinnest() * SHRINK_SHARE).max(0.0);
    solid.shrunk(by)
}

struct Limb {
    node: NodeId,
    solids: Vec<Solid>,
    points: Vec<Vec3>,
    low: Vec3,
    high: Vec3,
}

impl Limb {
    fn meets(&self, other: &Limb) -> bool {
        self.low.cmple(other.high).all() && other.low.cmple(self.high).all()
    }

    fn holds(&self, point: Vec3) -> bool {
        self.solids
            .iter()
            .any(|solid| solid.distance(point) <= ENCLOSED)
    }
}

fn limbs(clouds: Vec<(NodeId, Vec<Vec3>)>) -> Vec<Limb> {
    let mut limbs: Vec<Limb> = clouds
        .into_iter()
        .filter_map(|(node, points)| {
            let solids = fit::solids(&points);
            let (low, high) = solids.iter().map(Solid::bounds).fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(low, high), (from, to)| (low.min(from), high.max(to)),
            );
            (!solids.is_empty()).then_some(Limb {
                node,
                solids,
                points,
                low,
                high,
            })
        })
        .collect();
    let mut order: Vec<usize> = (0..limbs.len()).collect();
    order.sort_by_key(|&index| (limbs[index].points.len(), index));
    let mut retain = vec![true; limbs.len()];
    for index in order {
        let limb = &limbs[index];
        let around: Vec<&Limb> = limbs
            .iter()
            .enumerate()
            .filter(|&(other, other_limb)| {
                other != index && retain[other] && other_limb.meets(limb)
            })
            .map(|(_, other_limb)| other_limb)
            .collect();
        retain[index] = around.is_empty()
            || !limb
                .points
                .iter()
                .all(|point| around.iter().any(|other| other.holds(*point)));
    }
    let mut retain = retain.into_iter();
    limbs.retain(|_| retain.next().unwrap_or(false));
    limbs
}

fn owners(skeleton: &Skeleton, mut counts: Vec<usize>) -> Vec<Option<NodeId>> {
    let count = counts.len();
    let mut folded = vec![false; count];
    for node in skeleton.order.iter().rev() {
        let points = counts[node.index()];
        if points == 0 || points >= FEWEST {
            continue;
        }
        if let Some(parent) = skeleton.parents[node.index()] {
            counts[parent.index()] += points;
            counts[node.index()] = 0;
            folded[node.index()] = true;
        }
    }
    let mut owners: Vec<Option<NodeId>> = vec![None; count];
    for node in &skeleton.order {
        owners[node.index()] = match (folded[node.index()], skeleton.parents[node.index()]) {
            (true, Some(parent)) => owners[parent.index()],
            _ => Some(*node),
        };
    }
    for owner in &mut owners {
        *owner = owner.filter(|owner| counts[owner.index()] >= FEWEST);
    }
    owners
}
