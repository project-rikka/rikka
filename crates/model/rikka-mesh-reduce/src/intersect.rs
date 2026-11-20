use glam::{Affine3A, Vec3};
use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, IdMap, IdSet, LoopId, SelectMode, VertId};
use rikka_spatial::{Aabb, Bvh};

use crate::common::{Counts, Removed, any_face_selected};
use crate::doubles::AutoMergeOptions;
use crate::error::{Error, Result};
use crate::math::{AxisProjection, intersect_rays, is_point_in_polygon, ray_point_factor};
use crate::weld::WeldOptions;

const AXES: [Vec3; 7] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(1.0, 1.0, 1.0),
    Vec3::new(1.0, -1.0, 1.0),
    Vec3::new(1.0, 1.0, -1.0),
    Vec3::new(1.0, -1.0, -1.0),
];

const AXIS_LENGTHS: [f32; 7] = [
    1.0,
    1.0,
    1.0,
    1.732_050_8,
    1.732_050_8,
    1.732_050_8,
    1.732_050_8,
];

#[derive(Clone, Copy, Debug)]
struct Kdop {
    min: [f32; 7],
    max: [f32; 7],
}

impl Kdop {
    fn new(points: &[Vec3], epsilon: f32) -> Kdop {
        let mut min = [f32::MAX; 7];
        let mut max = [-f32::MAX; 7];
        for point in points {
            for axis in 0..7 {
                let projected =
                    point.x * AXES[axis].x + point.y * AXES[axis].y + point.z * AXES[axis].z;
                min[axis] = projected.min(min[axis]);
                max[axis] = projected.max(max[axis]);
            }
        }
        for axis in 0..7 {
            let corrected = epsilon * AXIS_LENGTHS[axis];
            min[axis] -= corrected;
            max[axis] += corrected;
        }
        Kdop { min, max }
    }

    fn overlaps(&self, other: &Kdop) -> bool {
        (0..7).all(|axis| !(self.min[axis] > other.max[axis] || other.min[axis] > self.max[axis]))
    }

    fn bounds(&self) -> Aabb {
        let slack = Vec3::splat(1.0e-6);
        let min = Vec3::new(self.min[0], self.min[1], self.min[2]);
        let max = Vec3::new(self.max[0], self.max[1], self.max[2]);
        let padding = (max - min).abs() * 1.0e-6 + slack;
        Aabb::new(min - padding, max + padding)
    }
}

struct Tree<T> {
    items: Vec<T>,
    kdops: Vec<Kdop>,
    bvh: Bvh,
}

impl<T: Copy> Tree<T> {
    fn new(items: Vec<T>, kdops: Vec<Kdop>) -> Option<Tree<T>> {
        if items.is_empty() {
            return None;
        }
        let bounds: Vec<Aabb> = kdops.iter().map(Kdop::bounds).collect();
        let bvh = Bvh::new(&bounds);
        Some(Tree { items, kdops, bvh })
    }

    fn self_pairs(&self) -> Vec<(T, T)> {
        let mut found: Vec<(u32, u32)> = Vec::new();
        self.bvh.for_each_self_overlap(|a, b| {
            if a != b && self.kdops[a as usize].overlaps(&self.kdops[b as usize]) {
                found.push((a.min(b), a.max(b)));
            }
        });
        found.sort_unstable();
        found.dedup();
        found
            .into_iter()
            .map(|(a, b)| (self.items[a as usize], self.items[b as usize]))
            .collect()
    }

    fn pairs<U: Copy>(&self, other: &Tree<U>) -> Vec<(T, U)> {
        let mut found: Vec<(u32, u32)> = Vec::new();
        self.bvh
            .for_each_overlap(&other.bvh, &Affine3A::IDENTITY, |a, b| {
                if self.kdops[a as usize].overlaps(&other.kdops[b as usize]) {
                    found.push((a, b));
                }
            });
        found.sort_unstable();
        found.dedup();
        found
            .into_iter()
            .map(|(a, b)| (self.items[a as usize], other.items[b as usize]))
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
enum Elem {
    Vert(VertId),
    Edge { edge: EdgeId, lambda: f32 },
}

impl Elem {
    fn vert(self) -> Option<VertId> {
        match self {
            Elem::Vert(vert) => Some(vert),
            Elem::Edge { .. } => None,
        }
    }
}

pub fn auto_merge_split(
    mesh: &mut EditMesh,
    moved: &IdSet<VertId>,
    options: &AutoMergeOptions,
) -> Result<Removed> {
    if !(options.threshold >= 0.0 && options.threshold.is_finite()) {
        return Err(Error::OutOfRange("threshold"));
    }
    let counts = Counts::new(mesh);
    let had_face_selection = any_face_selected(mesh);
    if let Some(targets) = intersect_edges(mesh, moved, options.threshold, true) {
        mesh.weld_verts(&targets, &WeldOptions::default());
    }
    if had_face_selection && mesh.select_mode() == SelectMode::FACE {
        mesh.select_mode_flush_with(SelectMode::EDGE, false);
    }
    mesh.update_normals();
    Ok(counts.removed(mesh))
}

pub fn intersect_edges(
    mesh: &mut EditMesh,
    moved: &IdSet<VertId>,
    distance: f32,
    split_faces: bool,
) -> Option<IdMap<VertId, VertId>> {
    let distance_squared = distance * distance;
    let half = distance / 2.0;
    let mut active_verts: Vec<VertId> = Vec::new();
    let mut remain_verts: Vec<VertId> = Vec::new();
    let verts: Vec<VertId> = mesh.verts().collect();
    for &vert in &verts {
        let active = moved.contains(vert);
        mesh.set_vert_tag(vert, active);
        if active {
            active_verts.push(vert);
        } else if !mesh.is_vert_hidden(vert) {
            remain_verts.push(vert);
        }
    }
    let vert_tree = |mesh: &EditMesh, list: Vec<VertId>| {
        let kdops = list
            .iter()
            .map(|&vert| Kdop::new(&[mesh.position(vert)], half))
            .collect();
        Tree::new(list, kdops)
    };
    let active_tree = vert_tree(mesh, active_verts);
    let remain_tree = vert_tree(mesh, remain_verts);

    let mut pairs: Vec<[Elem; 2]> = Vec::new();
    if let Some(active) = &active_tree {
        for (a, b) in active.self_pairs() {
            pairs.push([Elem::Vert(a), Elem::Vert(b)]);
        }
        if let Some(remain) = &remain_tree {
            for (a, b) in remain.pairs(active) {
                pairs.push([Elem::Vert(a), Elem::Vert(b)]);
            }
        }
    }

    let mut cuts: IdMap<EdgeId, usize> = IdMap::new();
    let mut active_edges: Vec<EdgeId> = Vec::new();
    let mut remain_edges: Vec<EdgeId> = Vec::new();
    let edges: Vec<EdgeId> = mesh.edges().collect();
    for &edge in &edges {
        let [v1, v2] = mesh.edge_verts(edge);
        if mesh.is_edge_hidden(edge)
            || mesh.position(v1).distance_squared(mesh.position(v2)) < distance_squared
        {
            if split_faces {
                mesh.set_edge_tag(edge, true);
            }
            continue;
        }
        if mesh.is_vert_tagged(v1) || mesh.is_vert_tagged(v2) {
            active_edges.push(edge);
        } else {
            remain_edges.push(edge);
            if split_faces {
                mesh.set_edge_tag(edge, true);
            }
        }
    }
    let has_active_edges = !active_edges.is_empty();
    let edge_tree = |mesh: &EditMesh, list: Vec<EdgeId>| {
        let kdops = list
            .iter()
            .map(|&edge| {
                let [a, b] = mesh.edge_verts(edge);
                Kdop::new(&[mesh.position(a), mesh.position(b)], half)
            })
            .collect();
        Tree::new(list, kdops)
    };
    let active_edge_tree = edge_tree(mesh, active_edges);
    let remain_edge_tree =
        match !remain_edges.is_empty() && (has_active_edges || active_tree.is_some()) {
            true => edge_tree(mesh, remain_edges),
            false => None,
        };

    let mut edge_pairs: Vec<[Elem; 2]> = Vec::new();
    if active_edge_tree.is_some() || remain_edge_tree.is_some() {
        for edge in edges.iter().copied() {
            mesh.set_edge_tag(edge, false);
        }
        let mut record = |pair: [Elem; 2], cuts: &mut IdMap<EdgeId, usize>| {
            for elem in pair {
                if let Elem::Edge { edge, .. } = elem {
                    *cuts.entry_or_insert_with(edge, || 0) += 1;
                }
            }
            edge_pairs.push(pair);
        };
        if let Some(active) = &active_edge_tree {
            for (a, b) in active.self_pairs() {
                if let Some(pair) = edge_edge(mesh, a, b, distance_squared) {
                    record(pair, &mut cuts);
                }
            }
            if let Some(remain) = &remain_edge_tree {
                for (a, b) in remain.pairs(active) {
                    if let Some(pair) = edge_edge(mesh, a, b, distance_squared) {
                        record(pair, &mut cuts);
                    }
                }
            }
            for tree in [&active_tree, &remain_tree].into_iter().flatten() {
                for (edge, vert) in active.pairs(tree) {
                    if let Some(pair) = edge_vert(mesh, edge, vert, distance_squared) {
                        record(pair, &mut cuts);
                    }
                }
            }
        }
        if let (Some(remain), Some(active)) = (&remain_edge_tree, &active_tree) {
            for (edge, vert) in remain.pairs(active) {
                if let Some(pair) = edge_vert(mesh, edge, vert, distance_squared) {
                    record(pair, &mut cuts);
                }
            }
        }
    }

    let vert_pair_count = pairs.len();
    pairs.extend(edge_pairs);
    let mut flat: Vec<Elem> = pairs.iter().flatten().copied().collect();

    if !cuts.is_empty() {
        let mut order: Vec<EdgeId> = Vec::new();
        let mut per_edge: IdMap<EdgeId, Vec<usize>> = IdMap::new();
        for (index, elem) in flat.iter().enumerate().skip(vert_pair_count * 2) {
            if let Elem::Edge { edge, .. } = *elem {
                if !per_edge.contains(edge) {
                    order.push(edge);
                    mesh.set_edge_tag(edge, true);
                }
                per_edge.entry_or_insert_with(edge, Vec::new).push(index);
            }
        }
        for edge in order {
            let Some(indices) = per_edge.remove(edge) else {
                continue;
            };
            let mut cuts: Vec<(usize, f32)> = indices
                .into_iter()
                .map(|index| match flat[index] {
                    Elem::Edge { lambda, .. } => (index, lambda),
                    Elem::Vert(_) => (index, 0.0),
                })
                .collect();
            cuts.sort_by(|a, b| a.1.total_cmp(&b.1));
            let mut previous = 0.0f32;
            for (index, lambda) in cuts {
                let relative = (lambda - previous) / (1.0 - previous);
                previous = lambda;
                if split_faces {
                    mesh.set_edge_tag(edge, false);
                }
                let start = mesh.edge_verts(edge)[0];
                let Ok(split) = mesh.split_edge(edge, start, relative) else {
                    return None;
                };
                flat[index] = Elem::Vert(split.vert);
            }
            let around: Vec<FaceId> = mesh.edge_faces(edge).collect();
            for face in around {
                mesh.update_face_normal(face);
            }
        }
    }

    if flat.is_empty() {
        return None;
    }
    let pair_count = flat.len() / 2;
    let mut targets: IdMap<VertId, VertId> = IdMap::new();
    for index in 0..pair_count {
        if let (Some(key), Some(value)) = (flat[index * 2].vert(), flat[index * 2 + 1].vert()) {
            targets.insert(key, value);
        }
    }
    let mut stamps: IdMap<VertId, usize> = IdMap::new();
    let limit = targets.len() + 1;
    for index in 0..pair_count {
        let (Some(key), Some(value)) = (flat[index * 2].vert(), flat[index * 2 + 1].vert()) else {
            continue;
        };
        let mut resolved = value;
        let mut steps = 0;
        while let Some(next) = targets.copied(resolved) {
            resolved = next;
            steps += 1;
            if steps > limit {
                break;
            }
        }
        if resolved != value {
            targets.insert(key, resolved);
            flat[index * 2 + 1] = Elem::Vert(resolved);
        }
        if split_faces {
            stamps.insert(key, index * 2);
            stamps.insert(resolved, index * 2 + 1);
        }
    }
    drop_invalid_targets(&mut targets);
    if split_faces {
        split_faces_along(mesh, &flat, &stamps, distance);
    }
    Some(targets)
}

fn drop_invalid_targets(targets: &mut IdMap<VertId, VertId>) {
    let invalid: Vec<VertId> = targets
        .iter()
        .filter(|&(key, &value)| key == value || targets.contains(value))
        .map(|(key, _)| key)
        .collect();
    for key in invalid {
        targets.remove(key);
    }
}

fn edge_vert(
    mesh: &EditMesh,
    edge: EdgeId,
    vert: VertId,
    distance_squared: f32,
) -> Option<[Elem; 2]> {
    let [v1, v2] = mesh.edge_verts(edge);
    let origin = mesh.position(v1);
    let direction = mesh.position(v2) - origin;
    let lambda = ray_point_factor(mesh.position(vert), origin, direction, -1.0);
    if !(0.0..=1.0).contains(&lambda) {
        return None;
    }
    let (end, factor) = match lambda < 0.5 {
        true => (v1, lambda),
        false => (v2, 1.0 - lambda),
    };
    if vert == end {
        return None;
    }
    if factor * factor * direction.length_squared() < distance_squared {
        return None;
    }
    let closest = origin + direction * lambda;
    if mesh.position(vert).distance_squared(closest) < distance_squared {
        return Some([Elem::Edge { edge, lambda }, Elem::Vert(vert)]);
    }
    None
}

fn edge_edge(mesh: &EditMesh, a: EdgeId, b: EdgeId, distance_squared: f32) -> Option<[Elem; 2]> {
    if mesh.edge_share_vert(a, b).is_some() {
        return None;
    }
    let [a1, a2] = mesh.edge_verts(a);
    let [b1, b2] = mesh.edge_verts(b);
    let origin_a = mesh.position(a1);
    let direction_a = mesh.position(a2) - origin_a;
    let origin_b = mesh.position(b1);
    let direction_b = mesh.position(b2) - origin_b;
    let (lambda_a, lambda_b) = intersect_rays(
        origin_a,
        direction_a,
        origin_b,
        direction_b,
        distance_squared * distance_squared,
    )?;
    let (end_a, factor_a) = match lambda_a < 0.5 {
        true => (a1, lambda_a),
        false => (a2, 1.0 - lambda_a),
    };
    let (end_b, factor_b) = match lambda_b < 0.5 {
        true => (b1, lambda_b),
        false => (b2, 1.0 - lambda_b),
    };
    if end_a == end_b {
        return None;
    }
    if !(0.0..=1.0).contains(&lambda_a) || !(0.0..=1.0).contains(&lambda_b) {
        return None;
    }
    if factor_a * factor_a * direction_a.length_squared() < distance_squared
        || factor_b * factor_b * direction_b.length_squared() < distance_squared
    {
        return None;
    }
    let near_a = origin_a + direction_a * lambda_a;
    let near_b = origin_b + direction_b * lambda_b;
    if near_a.distance_squared(near_b) < distance_squared {
        return Some([
            Elem::Edge {
                edge: a,
                lambda: lambda_a,
            },
            Elem::Edge {
                edge: b,
                lambda: lambda_b,
            },
        ]);
    }
    None
}

fn partner(flat: &[Elem], index: usize) -> Option<VertId> {
    let other = match index % 2 {
        1 => index - 1,
        _ => index + 1,
    };
    flat.get(other).and_then(|elem| elem.vert())
}

fn split_faces_along(
    mesh: &mut EditMesh,
    flat: &[Elem],
    stamps: &IdMap<VertId, usize>,
    distance: f32,
) {
    let mut index = 0;
    while index < mesh.edge_capacity() {
        let edge = EdgeId::new(index as u32);
        index += 1;
        if !mesh.contains_edge(edge) || mesh.is_edge_tagged(edge) {
            continue;
        }
        let [mut va, mut vb] = mesh.edge_verts(edge);
        let mut cut = stamps.copied(va);
        let mut cut_other = stamps.copied(vb);
        if cut.is_none() && cut_other.is_none() {
            if !mesh.is_vert_tagged(va) && !mesh.is_vert_tagged(vb) {
                mesh.set_edge_tag(edge, true);
            }
            continue;
        }
        mesh.set_edge_tag(edge, true);
        if cut.is_none() {
            std::mem::swap(&mut va, &mut vb);
            cut = cut_other;
            cut_other = None;
        }
        let Some(cut) = cut else {
            continue;
        };
        let Some(va_dest) = partner(flat, cut) else {
            continue;
        };
        if mesh.vert_pair_shares_face(va, va_dest) {
            continue;
        }
        let mut best_face: Option<FaceId> = None;
        let mut other = vb;
        let mut net_edge = edge;
        let mut net: Vec<EdgeId> = Vec::new();
        loop {
            let other_dest = match cut_other {
                Some(cut_other) => {
                    let Some(dest) = partner(flat, cut_other) else {
                        break;
                    };
                    if mesh.vert_pair_shares_face(other, dest) {
                        break;
                    }
                    dest
                }
                None => other,
            };
            if va_dest == other_dest {
                break;
            }
            if net.is_empty() && mesh.edge_between(va_dest, other_dest).is_some() {
                break;
            }
            net.push(net_edge);
            best_face = best_split_face(mesh, va_dest, other_dest, &net, distance);
            if best_face.is_some() {
                if va_dest != va && mesh.edge_between(va_dest, va).is_none() {
                    let first = net[0];
                    let far = match net.len() > 1 {
                        true => mesh.edge_other_vert(first, va).unwrap_or(other_dest),
                        false => other_dest,
                    };
                    if let Ok(created) = mesh.add_edge_like(va_dest, far, first) {
                        net[0] = created;
                    }
                }
                if net.len() > 1
                    && other_dest != other
                    && mesh.edge_between(other_dest, other).is_none()
                {
                    let last_index = net.len() - 1;
                    let last = net[last_index];
                    if let Some(near) = mesh.edge_other_vert(last, other)
                        && let Ok(created) = mesh.add_edge_like(other_dest, near, last)
                    {
                        net[last_index] = created;
                    }
                }
                break;
            }
            let mut next_edge: Option<EdgeId> = None;
            let mut probe = net_edge;
            loop {
                probe = mesh.disk_next(probe, other);
                if probe == net_edge {
                    break;
                }
                if !mesh.is_edge_wire(probe) {
                    if mesh.is_edge_tagged(probe) {
                        continue;
                    }
                    let [p1, p2] = mesh.edge_verts(probe);
                    if !mesh.is_vert_tagged(p1) && !mesh.is_vert_tagged(p2) {
                        continue;
                    }
                    mesh.set_edge_tag(probe, true);
                } else if !mesh.is_edge_wire(net_edge) {
                    continue;
                }
                next_edge = Some(probe);
                break;
            }
            let Some(next_edge) = next_edge else {
                break;
            };
            net_edge = next_edge;
            let Some(next) = mesh.edge_other_vert(net_edge, other) else {
                break;
            };
            other = next;
            if other == va {
                break;
            }
            cut_other = stamps.copied(other);
        }
        if let Some(face) = best_face {
            split_face_by_path(mesh, face, &net);
        }
    }
}

fn best_split_face(
    mesh: &EditMesh,
    a: VertId,
    b: VertId,
    net: &[EdgeId],
    epsilon: f32,
) -> Option<FaceId> {
    if a == b || mesh.vert_first_edge(a).is_none() || mesh.vert_first_edge(b).is_none() {
        return None;
    }
    if net.len() == 1 {
        let origin = mesh.position(b);
        let direction = mesh.position(a) - origin;
        for la in mesh.vert_loops(a).collect::<Vec<_>>() {
            let face = mesh.loop_face(la);
            let Some(lb) = mesh.face_vert_share_loop(face, b) else {
                continue;
            };
            if mesh.is_loop_adjacent(la, lb) {
                continue;
            }
            if splits_between(mesh, origin, direction, la, lb) {
                return Some(face);
            }
        }
        return None;
    }
    let mut best: Option<FaceId> = None;
    let mut best_range = f32::MAX;
    let first = net[0];
    let second = net[1];
    let [f1, f2] = mesh.edge_verts(first);
    let [s1, s2] = mesh.edge_verts(second);
    let start = match f1 == s1 || f1 == s2 {
        true => f2,
        false => f1,
    };
    for la in mesh.vert_loops(a).collect::<Vec<_>>() {
        let face = mesh.loop_face(la);
        let Some(lb) = mesh.face_vert_share_loop(face, b) else {
            continue;
        };
        let normal = mesh.face_normal(face);
        let mut min = mesh.position(mesh.loop_vert(la)).dot(normal);
        let mut max = mesh.position(mesh.loop_vert(lb)).dot(normal);
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        let mut probe = start;
        let mut inside = true;
        for &edge in &net[..net.len() - 1] {
            let Some(next) = mesh.edge_other_vert(edge, probe) else {
                inside = false;
                break;
            };
            probe = next;
            let point = mesh.position(probe);
            if !is_point_in_face(mesh, face, point) {
                inside = false;
                break;
            }
            let dot = point.dot(normal);
            min = min.min(dot);
            max = max.max(dot);
        }
        if !inside {
            continue;
        }
        let range = max - min;
        if range < best_range {
            best_range = range;
            best = Some(face);
        }
    }
    if let Some(face) = best {
        let normal = mesh.face_normal(face);
        let mut min = f32::MAX;
        let mut max = -f32::MAX;
        for vert in mesh.face_verts(face) {
            let dot = mesh.position(vert).dot(normal);
            min = min.min(dot);
            max = max.max(dot);
        }
        if max - min + 2.0 * epsilon < best_range {
            return None;
        }
    }
    best
}

fn splits_between(mesh: &EditMesh, origin: Vec3, direction: Vec3, la: LoopId, lb: LoopId) -> bool {
    let range = -f32::EPSILON..=1.0 + f32::EPSILON;
    let segment = |lp: LoopId| {
        let start = mesh.position(mesh.loop_vert(mesh.loop_prev(lp)));
        let end = mesh.position(mesh.loop_vert(mesh.loop_next(lp)));
        (start, end - start)
    };
    let (start, along) = segment(la);
    let Some((_, lambda)) = intersect_rays(origin, direction, start, along, f32::MIN_POSITIVE)
    else {
        return false;
    };
    if range.contains(&lambda) {
        return true;
    }
    let (start, along) = segment(lb);
    match intersect_rays(origin, direction, start, along, f32::MIN_POSITIVE) {
        Some((_, lambda)) => range.contains(&lambda),
        None => false,
    }
}

fn is_point_in_face(mesh: &EditMesh, face: FaceId, point: Vec3) -> bool {
    let projection = AxisProjection::new(mesh.face_normal(face));
    let polygon: Vec<_> = mesh
        .face_verts(face)
        .map(|vert| projection.project(mesh.position(vert)))
        .collect();
    is_point_in_polygon(projection.project(point), &polygon)
}

fn split_face_by_path(mesh: &mut EditMesh, face: FaceId, net: &[EdgeId]) {
    let Some(path) = net_path(mesh, face, net) else {
        return;
    };
    let (Some(&start), Some(&end)) = (path.first(), path.last()) else {
        return;
    };
    let Some(l_start) = mesh.face_vert_share_loop(face, start) else {
        return;
    };
    let Some(l_end) = mesh.face_vert_share_loop(face, end) else {
        return;
    };
    let interior = &path[1..path.len() - 1];
    let boundary = |from: LoopId, to: LoopId| {
        let mut verts = Vec::new();
        let mut lp = from;
        loop {
            verts.push(mesh.loop_vert(lp));
            if lp == to {
                break;
            }
            lp = mesh.loop_next(lp);
        }
        verts
    };
    let mut side_a = boundary(l_start, l_end);
    side_a.extend(interior.iter().rev().copied());
    let mut side_b = boundary(l_end, l_start);
    side_b.extend(interior.iter().copied());
    if side_a.len() < 3 || side_b.len() < 3 {
        return;
    }
    let first_loop = mesh.face_first_loop(face);
    let first_vert = mesh.loop_vert(first_loop);
    let first_edge = mesh.loop_edge(first_loop);
    let has_first_edge = |side: &[VertId]| {
        (0..side.len()).any(|index| {
            mesh.edge_between(side[index], side[(index + 1) % side.len()]) == Some(first_edge)
        })
    };
    let a_first = match (side_a.contains(&first_vert), side_b.contains(&first_vert)) {
        (true, false) => true,
        (false, true) => false,
        _ => has_first_edge(&side_a),
    };
    let (mut first_side, mut second_side) = match a_first {
        true => (side_a, side_b),
        false => (side_b, side_a),
    };
    rotate_to(&mut first_side, first_vert);
    if let Some(&start) = first_side.iter().rev().find(|vert| path.contains(vert)) {
        rotate_to(&mut second_side, start);
    }
    let sample = mesh.sample_face(face);
    let flags = mesh.face_flags(face);
    let mut created: Vec<FaceId> = Vec::new();
    for side in [&first_side, &second_side] {
        if mesh.face_exists(side).is_some() {
            continue;
        }
        let Ok(new) = mesh.new_face(side).existing_edges().example(face).create() else {
            continue;
        };
        mesh.set_face_flags(new, flags);
        created.push(new);
    }
    if created.is_empty() {
        return;
    }
    let original: Vec<LoopId> = mesh.face_loops_vec(face);
    for &new in &created {
        for lp in mesh.face_loops_vec(new) {
            let vert = mesh.loop_vert(lp);
            match original
                .iter()
                .find(|&&source| mesh.loop_vert(source) == vert)
            {
                Some(&source) => mesh.copy_loop_data(source, lp),
                None => mesh.interpolate_loop_from_sample(lp, &sample, false),
            }
        }
    }
    let active = mesh.active_face() == Some(face);
    if mesh.kill_face(face).is_err() {
        return;
    }
    if let Ok(copy) = mesh.copy_face(created[0], false, false) {
        mesh.set_face_flags(copy, flags);
        mesh.kill_face(created[0]).ok();
        created[0] = copy;
    }
    for &new in &created {
        mesh.update_face_normal(new);
    }
    if active {
        mesh.set_active_face(Some(created[0]));
    }
}

fn net_path(mesh: &EditMesh, face: FaceId, net: &[EdgeId]) -> Option<Vec<VertId>> {
    let in_face = |vert: VertId| mesh.is_vert_in_face(vert, face);
    let first = *net.first()?;
    let [f1, f2] = mesh.edge_verts(first);
    let mut start = match (in_face(f1), in_face(f2)) {
        (true, _) => f1,
        (false, true) => f2,
        (false, false) => return None,
    };
    if net.len() > 1 {
        let [s1, s2] = mesh.edge_verts(net[1]);
        start = match f1 == s1 || f1 == s2 {
            true => f2,
            false => f1,
        };
    }
    let mut path = vec![start];
    let mut current = start;
    for &edge in net {
        current = mesh.edge_other_vert(edge, current)?;
        path.push(current);
    }
    let (&head, &tail) = (path.first()?, path.last()?);
    if head == tail || !in_face(head) || !in_face(tail) {
        return None;
    }
    if path[1..path.len() - 1].iter().any(|&vert| in_face(vert)) {
        return None;
    }
    Some(path)
}

fn rotate_to(side: &mut [VertId], start: VertId) {
    if let Some(position) = side.iter().position(|&vert| vert == start) {
        side.rotate_left(position);
    }
}
