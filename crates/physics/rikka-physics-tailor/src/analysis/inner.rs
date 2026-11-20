use std::collections::HashMap;
use std::f32::consts::TAU;

use glam::Vec3;
use rikka_model_core::{HumanBone, NodeId};

use crate::figure::Figure;

const ALONG: f32 = 0.015;
const SECTORS: f32 = 16.0;
const SLACK: f32 = 0.005;
const LEAST: f32 = 0.01;
const FACIAL: [HumanBone; 3] = [HumanBone::LeftEye, HumanBone::RightEye, HumanBone::Jaw];

#[derive(Clone, Copy, Debug)]
struct Segment {
    start: Vec3,
    axis: Vec3,
    length: f32,
    across: Vec3,
    other: Vec3,
}

impl Segment {
    fn new(start: Vec3, axis: Vec3, length: f32) -> Segment {
        let axis = axis.normalize_or(Vec3::Y);
        let (across, other) = axis.any_orthonormal_pair();
        Segment {
            start,
            axis,
            length: length.max(LEAST),
            across,
            other,
        }
    }

    fn radial(&self, point: Vec3) -> (f32, Vec3) {
        let offset = point - self.start;
        let along = offset.dot(self.axis).clamp(0.0, self.length);
        (along, offset - self.axis * along)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Bin {
    segment: usize,
    along: i32,
    sector: u8,
}

pub(crate) fn innermost(figure: &Figure<'_>, carried: &[bool]) -> Vec<bool> {
    let segments = segments(figure, carried);
    let mut inner = vec![false; figure.vertices.len()];
    if segments.is_empty() {
        return inner;
    }
    let placed: Vec<Option<(Bin, f32)>> = figure
        .vertices
        .iter()
        .map(|vertex| {
            if carried[vertex.strongest().index()] {
                return None;
            }
            let (index, (along, radial)) = segments
                .iter()
                .enumerate()
                .map(|(index, segment)| (index, segment.radial(vertex.position)))
                .min_by(|a, b| a.1.1.length_squared().total_cmp(&b.1.1.length_squared()))?;
            let segment = &segments[index];
            let turn = radial
                .dot(segment.other)
                .atan2(radial.dot(segment.across))
                .rem_euclid(TAU);
            let bin = Bin {
                segment: index,
                along: (along / ALONG).floor() as i32,
                sector: (turn / TAU * SECTORS) as u8,
            };
            Some((bin, radial.length()))
        })
        .collect();
    let mut nearest: HashMap<Bin, f32> = HashMap::new();
    for (bin, radial) in placed.iter().flatten() {
        let smallest = nearest.entry(*bin).or_insert(f32::MAX);
        *smallest = smallest.min(*radial);
    }
    for (vertex, slot) in placed.iter().enumerate() {
        if let Some((bin, radial)) = slot {
            inner[vertex] = *radial <= nearest[bin] + SLACK;
        }
    }
    inner
}

fn segments(figure: &Figure<'_>, carried: &[bool]) -> Vec<Segment> {
    let anatomy = &figure.anatomy;
    let skeleton = &figure.skeleton;
    let bones: Vec<(HumanBone, NodeId)> = anatomy
        .humanoid()
        .bones()
        .filter(|(bone, _)| !FACIAL.contains(bone))
        .collect();
    let human = |node: NodeId| bones.iter().any(|(_, bone)| *bone == node);
    let upper = |node: NodeId| {
        let mut ancestor = skeleton.parents[node.index()];
        for _ in 0..skeleton.parents.len() {
            let current = ancestor?;
            if human(current) {
                return Some(current);
            }
            ancestor = skeleton.parents[current.index()];
        }
        None
    };
    let origin = |node: NodeId| skeleton.origins[node.index()];
    let uppers: Vec<Option<NodeId>> = bones.iter().map(|(_, node)| upper(*node)).collect();
    let mut segments = Vec::new();
    let mut leaves: Vec<(HumanBone, Vec3, Vec3)> = Vec::new();
    for ((bone, node), above) in bones.iter().zip(&uppers) {
        let start = origin(*node);
        let mut lower = bones
            .iter()
            .zip(&uppers)
            .filter(|(_, above)| **above == Some(*node))
            .map(|((_, child), _)| origin(*child))
            .peekable();
        if lower.peek().is_none() {
            let axis = above
                .map(|parent| start - origin(parent))
                .unwrap_or(Vec3::Y)
                .normalize_or(Vec3::Y);
            leaves.push((*bone, start, axis));
        }
        for end in lower {
            segments.push(Segment::new(start, end - start, end.distance(start)));
        }
    }
    let mut lengths = vec![0.0f32; leaves.len()];
    for vertex in &figure.vertices {
        let strongest = vertex.strongest();
        if carried[strongest.index()] {
            continue;
        }
        let Some(region) = anatomy.region(strongest) else {
            continue;
        };
        if let Some(leaf) = leaves.iter().position(|(bone, _, _)| *bone == region) {
            let (_, start, axis) = leaves[leaf];
            lengths[leaf] = lengths[leaf].max((vertex.position - start).dot(axis));
        }
    }
    segments.extend(
        leaves
            .iter()
            .zip(lengths)
            .map(|((_, start, axis), length)| Segment::new(*start, *axis, length)),
    );
    segments
}
