use std::collections::BTreeMap;

use super::class::Class;
use super::features::MeshFeatures;
use crate::cloth_settings::Role;
use crate::figure::RestVertex;
use crate::geometry::{PointGrid, Union, groups};

const RIGID_WEIGHT: f32 = 0.9;
const RIGID_SHARE: f32 = 0.9;

#[derive(Clone, Debug)]
pub(crate) struct Part {
    pub(crate) members: Vec<u32>,
    pub(crate) strands: bool,
    pub(crate) is_attachment: bool,
}

pub(crate) fn parts(
    features: &MeshFeatures<'_, '_>,
    roles: &mut [Role],
    strands: &[bool],
) -> Vec<Part> {
    let figure = features.figure;
    let chosen: Vec<bool> = roles
        .iter()
        .map(|role| matches!(role, Role::Free | Role::Pinned))
        .collect();
    let selected = |vertex: u32| chosen[vertex as usize];
    let mut union = Union::new(roles.len());
    for triangle in &figure.triangles {
        let [a, b, c] = triangle.corners;
        for (x, y) in [(a, b), (b, c), (c, a)] {
            if selected(x) && selected(y) {
                union.join(x, y);
            }
        }
    }
    let same = figure.same_distance();
    let grid = PointGrid::new(
        (0..roles.len() as u32)
            .filter(|vertex| selected(*vertex))
            .map(|vertex| (vertex, features.position(vertex))),
        same * 2.0,
    );
    for vertex in (0..roles.len() as u32).filter(|vertex| selected(*vertex)) {
        let at = features.position(vertex);
        for other in grid.near(at, same) {
            if other > vertex
                && strands[other as usize] == strands[vertex as usize]
                && features.position(other).distance(at) <= same
            {
                union.join(vertex, other);
            }
        }
    }
    let (labels, count) = union.labels();
    let mut parts = Vec::new();
    for members in groups(&labels, count) {
        if !selected(members[0]) {
            continue;
        }
        if !members
            .iter()
            .any(|vertex| roles[*vertex as usize] == Role::Free)
        {
            for vertex in &members {
                roles[*vertex as usize] = Role::Skinned;
            }
            continue;
        }
        let solid = members
            .iter()
            .filter(|vertex| features.class[**vertex as usize] == Class::Solid)
            .count()
            * 2
            > members.len();
        let strands = strands[members[0] as usize];
        parts.push(Part {
            strands,
            is_attachment: solid || rigid(features, &members, strands),
            members,
        });
    }
    parts
}

fn rigid(features: &MeshFeatures<'_, '_>, members: &[u32], strands: bool) -> bool {
    let vertices = &features.figure.vertices;
    let share = |predicate: &dyn Fn(&RestVertex) -> bool| {
        members
            .iter()
            .filter(|vertex| predicate(&vertices[**vertex as usize]))
            .count() as f32
            >= RIGID_SHARE * members.len() as f32
    };
    let carried = |vertex: &RestVertex| features.carried[vertex.strongest().index()];
    let one = majority(
        members
            .iter()
            .map(|vertex| vertices[*vertex as usize].strongest()),
    );
    let single_bone = share(&|vertex| carried(vertex) && Some(vertex.strongest()) == one);
    let segments = share(&|vertex| carried(vertex) && vertex.weights[0] >= RIGID_WEIGHT);
    single_bone || (segments && !strands)
}

pub(crate) fn majority<T: Copy + Ord>(items: impl Iterator<Item = T>) -> Option<T> {
    let mut counts: BTreeMap<T, usize> = BTreeMap::new();
    for item in items {
        *counts.entry(item).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(item, _)| item)
}
