use std::f32::consts::TAU;

use glam::Vec3;
use rikka_humanoid::Swing;
use rikka_model_core::{Bounds, HumanBone};

use super::features::MeshFeatures;
use crate::cloth_settings::Role;
use crate::geometry::Principal;
use crate::preset::Preset;

const SMALL: f32 = 0.06;
const NARROW: f32 = 0.045;
const LONG_HAIR: f32 = 0.22;
const BEHIND: f32 = 0.03;
const SHOULDERS: f32 = 0.25;
const SECTORS: usize = 16;
const WRAPPED: usize = 12;
const LIMB: f32 = 0.07;
const LIMB_MARGIN: f32 = 0.01;
const LIMB_POINTS: usize = 8;
const STANDING: f32 = 0.01;
const SLEEVE_SHARE: f32 = 0.5;
const TAIL_SHARE: f32 = 0.8;
const ROUND: f32 = 0.35;
const ELONGATED: f32 = 2.0;
const ONE_ROOT: f32 = 0.04;
const ARMS: [HumanBone; 6] = [
    HumanBone::LeftUpperArm,
    HumanBone::LeftLowerArm,
    HumanBone::LeftHand,
    HumanBone::RightUpperArm,
    HumanBone::RightLowerArm,
    HumanBone::RightHand,
];
const BACK: [HumanBone; 2] = [HumanBone::Hips, HumanBone::Spine];

pub(crate) struct Silhouette {
    points: Vec<(Vec3, bool)>,
}

impl Silhouette {
    pub(crate) fn measure(features: &MeshFeatures<'_, '_>, roles: &[Role]) -> Silhouette {
        let under = &features.figure.skeleton.under_head;
        let mut points: Vec<(Vec3, bool)> = features
            .figure
            .vertices
            .iter()
            .zip(roles)
            .filter(|(_, role)| **role == Role::Body)
            .map(|(vertex, _)| (vertex.position, under[vertex.strongest().index()]))
            .collect();
        points.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
        Silhouette { points }
    }

    fn axis_between(&self, low: f32, high: f32) -> Option<Vec3> {
        let start = self.points.partition_point(|(point, _)| point.y < low);
        let end = self.points.partition_point(|(point, _)| point.y <= high);
        let band = &self.points[start..end.max(start)];
        let points = if band.is_empty() {
            &self.points[..]
        } else {
            band
        };
        (!points.is_empty()).then(|| {
            let sum: Vec3 = points.iter().map(|(point, _)| *point).sum();
            let mean = sum / points.len() as f32;
            Vec3::new(mean.x, 0.0, mean.z)
        })
    }

    fn limb_in(&self, bounds: &Bounds) -> Option<Principal> {
        let low = bounds.min - LIMB_MARGIN;
        let high = bounds.max + LIMB_MARGIN;
        let start = self.points.partition_point(|(point, _)| point.y < low.y);
        let inside: Vec<Vec3> = self.points[start..]
            .iter()
            .take_while(|(point, _)| point.y <= high.y)
            .filter(|(point, head)| !head && point.cmpge(low).all() && point.cmple(high).all())
            .map(|(point, _)| *point)
            .collect();
        if inside.len() < LIMB_POINTS {
            return None;
        }
        Principal::compute(&inside).filter(|limb| limb.half[1] < LIMB)
    }
}

pub(crate) fn suggest_hair(
    features: &MeshFeatures<'_, '_>,
    members: &[u32],
    roles: &[Role],
) -> Preset {
    let (free, root) = free_and_root(features, members, roles);
    let length = free
        .iter()
        .map(|point| point.distance(root))
        .fold(0.0f32, f32::max);
    if length >= LONG_HAIR {
        return Preset::LongHair;
    }
    if let Some(head) = features.head {
        let toward = root - head.centre;
        let flat = Vec3::new(toward.x, 0.0, toward.z);
        if flat.dot(head.forward) > 0.5 * flat.length() && root.y > head.centre.y - BEHIND {
            return Preset::Bangs;
        }
    }
    Preset::ShortHair
}

pub(crate) fn stands(features: &MeshFeatures<'_, '_>, members: &[u32], roles: &[Role]) -> bool {
    let height = |role: Role| {
        let (sum, count) = members
            .iter()
            .filter(|vertex| roles[**vertex as usize] == role)
            .fold((0.0f32, 0usize), |(sum, count), vertex| {
                (sum + features.position(*vertex).y, count + 1)
            });
        (count > 0).then(|| sum / count as f32)
    };
    matches!((height(Role::Free), height(Role::Pinned)), (Some(free), Some(pinned)) if free > pinned + STANDING)
}

fn tail(features: &MeshFeatures<'_, '_>, free: &[Vec3], members: &[u32], roles: &[Role]) -> bool {
    if hung_share(features, members, roles, |bone| BACK.contains(&bone)) < TAIL_SHARE {
        return false;
    }
    let figure = features.figure;
    let roots: Vec<Vec3> = free_vertices(members, roles)
        .filter_map(|vertex| hung(features, vertex))
        .map(|swing| figure.skeleton.origins[swing.root().index()])
        .collect();
    let Some(first) = roots.first().copied() else {
        return false;
    };
    let anatomy = &figure.anatomy;
    let behind = anatomy.node(HumanBone::Hips).is_some_and(|hips| {
        let offset = first - figure.skeleton.origins[hips.index()];
        offset.dot(anatomy.forward()) < -BEHIND
    });
    let round = Principal::compute(free).is_some_and(|shape| {
        shape.half[2] >= ROUND * shape.half[1] && shape.half[0] >= ELONGATED * shape.half[1]
    });
    behind && round && roots.iter().all(|root| root.distance(first) <= ONE_ROOT)
}

fn hung_share(
    features: &MeshFeatures<'_, '_>,
    members: &[u32],
    roles: &[Role],
    carried_by: impl Fn(HumanBone) -> bool,
) -> f32 {
    let (count, carried_count) =
        free_vertices(members, roles).fold((0usize, 0usize), |(count, carried_count), vertex| {
            let carried = hung(features, vertex)
                .and_then(|swing| swing.bone())
                .is_some_and(&carried_by);
            (count + 1, carried_count + usize::from(carried))
        });
    carried_count as f32 / count.max(1) as f32
}

fn hung<'s>(features: &'s MeshFeatures<'_, '_>, vertex: u32) -> Option<&'s Swing> {
    let figure = features.figure;
    figure.hung(figure.vertices[vertex as usize].strongest())
}

fn free_vertices<'r>(members: &'r [u32], roles: &'r [Role]) -> impl Iterator<Item = u32> + 'r {
    members
        .iter()
        .copied()
        .filter(|vertex| roles[*vertex as usize] == Role::Free)
}

pub(crate) fn suggest_sheet(
    features: &MeshFeatures<'_, '_>,
    silhouette: &Silhouette,
    members: &[u32],
    roles: &[Role],
) -> Preset {
    let (free, root) = free_and_root(features, members, roles);
    if tail(features, &free, members, roles) {
        return Preset::Tail;
    }
    let bounds = Bounds::from_points(free.iter().copied());
    if bounds.longest() < SMALL {
        return Preset::Accessory;
    }
    if let Some(principal) = Principal::compute(&free)
        && principal.half[1] < NARROW
        && principal.half[0] > 3.0 * principal.half[1]
    {
        return Preset::Ribbon;
    }
    if hung_share(features, members, roles, |bone| ARMS.contains(&bone)) >= SLEEVE_SHARE {
        return Preset::Sleeve;
    }
    let body = silhouette.axis_between(bounds.min.y, bounds.max.y);
    if body.is_some_and(|centre| sectors(&free, centre, Vec3::Y) >= WRAPPED) {
        return Preset::Skirt;
    }
    if let Some(limb) = silhouette.limb_in(&bounds)
        && sectors(&free, limb.centre, limb.axes[0]) >= WRAPPED
    {
        return Preset::Sleeve;
    }
    if let (Some(head), Some(centre)) = (features.head, body) {
        let neck = features.figure.skeleton.origins[head.node.index()];
        let mean = free.iter().copied().sum::<Vec3>() / free.len().max(1) as f32;
        let flat = Vec3::new(mean.x - centre.x, 0.0, mean.z - centre.z);
        if root.y > neck.y - SHOULDERS {
            let forward = flat.dot(head.forward);
            if forward < -BEHIND {
                return Preset::Cape;
            }
            if forward > BEHIND {
                return Preset::Ribbon;
            }
        }
    }
    Preset::Skirt
}

fn free_and_root(
    features: &MeshFeatures<'_, '_>,
    members: &[u32],
    roles: &[Role],
) -> (Vec<Vec3>, Vec3) {
    let positions = |role: Role| -> Vec<Vec3> {
        members
            .iter()
            .filter(|vertex| roles[**vertex as usize] == role)
            .map(|vertex| features.position(*vertex))
            .collect()
    };
    let free = positions(Role::Free);
    let pins = positions(Role::Pinned);
    let mean = |points: &[Vec3]| points.iter().copied().sum::<Vec3>() / points.len().max(1) as f32;
    let root = if pins.is_empty() {
        free.iter()
            .copied()
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .unwrap_or_default()
    } else {
        mean(&pins)
    };
    (free, root)
}

fn sectors(points: &[Vec3], centre: Vec3, axis: Vec3) -> usize {
    let axis = axis.normalize_or(Vec3::Y);
    let (across, other) = axis.any_orthonormal_pair();
    let mut filled = [false; SECTORS];
    for point in points {
        let offset = *point - centre;
        let (x, y) = (offset.dot(across), offset.dot(other));
        if x * x + y * y < 1e-6 {
            continue;
        }
        let turn = y.atan2(x).rem_euclid(TAU) / TAU;
        filled[((turn * SECTORS as f32) as usize).min(SECTORS - 1)] = true;
    }
    filled.iter().filter(|sector| **sector).count()
}
