use rikka_polymesh::{Domain, MergeData, Polymesh, ordered};
use rikka_spatial::PointTree;
use rustc_hash::FxHashMap;

use super::vertex_group::Group;
use crate::kinds::weld::{Weld, WeldMode};
use crate::origins;

pub(crate) fn evaluate(weld: &Weld, mesh: &mut Polymesh) {
    let selected = selection(weld, mesh);
    let targets = match weld.mode {
        WeldMode::All => nearby_targets(weld, mesh, &selected),
        WeldMode::Connected => connected_targets(weld, mesh, &selected),
    };
    merge(mesh, &targets);
}

pub(crate) fn merge(mesh: &mut Polymesh, targets: &[u32]) {
    if targets
        .iter()
        .enumerate()
        .all(|(vertex, &target)| vertex as u32 == target)
    {
        return;
    }
    unify_origins(mesh, targets);
    mesh.merge_points(targets, MergeData::Mix);
}

fn unify_origins(mesh: &mut Polymesh, targets: &[u32]) {
    let root = |vertex: u32| {
        let mut vertex = vertex;
        while targets[vertex as usize] != vertex {
            vertex = targets[vertex as usize];
        }
        vertex
    };
    let roots: Vec<u32> = (0..targets.len() as u32).map(root).collect();
    origins::unify(mesh, Domain::Point, &roots);
    let mut first: FxHashMap<[u32; 2], u32> = FxHashMap::default();
    let edge_targets: Vec<u32> = mesh
        .edges
        .iter()
        .enumerate()
        .map(|(edge, &[a, b])| {
            *first
                .entry(ordered([roots[a as usize], roots[b as usize]]))
                .or_insert(edge as u32)
        })
        .collect();
    origins::unify(mesh, Domain::Edge, &edge_targets);
}

fn selection(weld: &Weld, mesh: &Polymesh) -> Vec<bool> {
    let vertices = mesh.point_count();
    match Group::find(mesh, &weld.vertex_group) {
        Group::Weighted(group) => (0..vertices)
            .map(|vertex| {
                (mesh.vertex_groups.weight(vertex, group) > 0.0) != weld.invert_vertex_group
            })
            .collect(),
        Group::Unweighted | Group::Unset => vec![true; vertices],
    }
}

fn nearby_targets(weld: &Weld, mesh: &Polymesh, selected: &[bool]) -> Vec<u32> {
    let tree = PointTree::from_indexed(
        mesh.positions
            .iter()
            .enumerate()
            .filter(|&(vertex, _)| selected[vertex])
            .map(|(vertex, &position)| (vertex as u32, position)),
    );
    let duplicates = tree.find_duplicates(weld.merge_threshold);
    (0..mesh.point_count() as u32)
        .map(|vertex| duplicates.target(vertex))
        .collect()
}

fn connected_targets(weld: &Weld, mesh: &Polymesh, selected: &[bool]) -> Vec<u32> {
    let mut targets: Vec<u32> = (0..mesh.point_count() as u32).collect();
    let mut used = vec![false; mesh.edges.len()];
    if weld.loose_edges {
        for &edge in &mesh.corner_edges {
            if let Some(flag) = used.get_mut(edge as usize) {
                *flag = true;
            }
        }
    }
    let mut centers = mesh.positions.clone();
    let mut merged = vec![0u32; mesh.positions.len()];
    let root = |targets: &[u32], vertex: u32| {
        let mut vertex = vertex;
        while targets[vertex as usize] != vertex {
            vertex = targets[vertex as usize];
        }
        vertex
    };
    let limit = weld.merge_threshold * weld.merge_threshold;
    for (index, &[a, b]) in mesh.edges.iter().enumerate() {
        if used[index] {
            continue;
        }
        let (first, second) = (root(&targets, a), root(&targets, b));
        if first == second || !selected[first as usize] || !selected[second as usize] {
            continue;
        }
        let (low, high) = (first.min(second) as usize, first.max(second) as usize);
        let along = centers[high] - centers[low];
        if along.length_squared() <= limit {
            let influence = (merged[high] + 1) as f32 / (merged[low] + merged[high] + 2) as f32;
            centers[low] += along * influence;
            merged[low] += merged[high] + 1;
            targets[high] = low as u32;
        }
    }
    for vertex in 0..targets.len() as u32 {
        targets[vertex as usize] = root(&targets, vertex);
    }
    targets
}
