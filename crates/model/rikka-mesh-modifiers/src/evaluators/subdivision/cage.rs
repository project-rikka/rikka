use std::sync::atomic::{AtomicU32, Ordering};

use glam::Vec2;
use rikka_polymesh::Polymesh;
use rustc_hash::FxHashMap;

use crate::PARALLEL;
use crate::kinds::subdivision::UvSmooth;

pub(super) const INFINITE: f32 = f32::INFINITY;
const SHARPNESS_INFINITE: f32 = 10.0;
pub(super) const NO_EDGE: u32 = u32::MAX;

#[derive(Clone)]
pub(super) struct Cage {
    pub(super) points: usize,
    pub(super) faces: Vec<u32>,
    pub(super) corners: Vec<u32>,
    pub(super) edges: Vec<[u32; 2]>,
    pub(super) corner_edges: Vec<u32>,
    pub(super) edge_sharpness: Vec<f32>,
    pub(super) point_sharpness: Vec<f32>,
}

impl Cage {
    pub(super) fn face_count(&self) -> usize {
        self.faces.len() - 1
    }

    pub(super) fn face(&self, face: usize) -> std::ops::Range<usize> {
        self.faces[face] as usize..self.faces[face + 1] as usize
    }

    pub(super) fn corner_faces(&self) -> Vec<u32> {
        let mut owners = vec![0u32; self.corners.len()];
        let face_ranges = PARALLEL.grain(GRAIN).ranges(self.face_count());
        let corner_ranges: Vec<std::ops::Range<usize>> = face_ranges
            .iter()
            .map(|range| self.faces[range.start] as usize..self.faces[range.end] as usize)
            .collect();
        PARALLEL.split_mut(&mut owners, &corner_ranges, |index, piece| {
            let base = corner_ranges[index].start;
            for face in face_ranges[index].clone() {
                let range = self.face(face);
                piece[range.start - base..range.end - base].fill(face as u32);
            }
        });
        owners
    }
}

pub(super) struct Grouped {
    offsets: Vec<u32>,
    values: Vec<u32>,
}

const SKIP: u32 = u32::MAX;
const GRAIN: usize = 1 << 14;

impl Grouped {
    pub(super) fn new(
        groups: usize,
        len: usize,
        entry: impl Fn(usize) -> (u32, u32) + Sync,
    ) -> Grouped {
        let counts: Vec<AtomicU32> = (0..=groups).map(|_| AtomicU32::new(0)).collect();
        PARALLEL.grain(GRAIN).map_chunks(len, |range| {
            for index in range {
                let (group, _) = entry(index);
                if group != SKIP {
                    counts[group as usize + 1].fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        let mut offsets: Vec<u32> = counts.into_iter().map(AtomicU32::into_inner).collect();
        for index in 0..groups {
            offsets[index + 1] += offsets[index];
        }
        let cursors: Vec<AtomicU32> = offsets[..groups]
            .iter()
            .map(|&offset| AtomicU32::new(offset))
            .collect();
        let slots: Vec<AtomicU32> = (0..offsets[groups]).map(|_| AtomicU32::new(0)).collect();
        PARALLEL.grain(GRAIN).map_chunks(len, |range| {
            for index in range {
                let (group, value) = entry(index);
                if group != SKIP {
                    let slot = cursors[group as usize].fetch_add(1, Ordering::Relaxed);
                    slots[slot as usize].store(value, Ordering::Relaxed);
                }
            }
        });
        let mut values: Vec<u32> = slots.into_iter().map(AtomicU32::into_inner).collect();
        let group_ranges = PARALLEL.grain(GRAIN).ranges(groups);
        let value_ranges: Vec<std::ops::Range<usize>> = group_ranges
            .iter()
            .map(|range| offsets[range.start] as usize..offsets[range.end] as usize)
            .collect();
        let offsets_ref = &offsets;
        PARALLEL.split_mut(&mut values, &value_ranges, |index, piece| {
            let base = value_ranges[index].start;
            for group in group_ranges[index].clone() {
                let start = offsets_ref[group] as usize - base;
                let end = offsets_ref[group + 1] as usize - base;
                piece[start..end].sort_unstable();
            }
        });
        Grouped { offsets, values }
    }

    pub(super) fn get(&self, group: usize) -> &[u32] {
        &self.values[self.offsets[group] as usize..self.offsets[group + 1] as usize]
    }
}

pub(super) struct Around {
    pub(super) edges: Grouped,
    pub(super) faces: Grouped,
    pub(super) edge_faces: Grouped,
}

impl Around {
    pub(super) fn new(cage: &Cage) -> Around {
        let corner_faces = cage.corner_faces();
        let edges = Grouped::new(cage.points, cage.edges.len() * 2, |index| {
            (cage.edges[index / 2][index % 2], (index / 2) as u32)
        });
        let faces = Grouped::new(cage.points, cage.corners.len(), |corner| {
            (cage.corners[corner], corner_faces[corner])
        });
        let edge_faces = Grouped::new(cage.edges.len(), cage.corners.len(), |corner| {
            match cage.corner_edges[corner] {
                NO_EDGE => (SKIP, 0),
                edge => (edge, corner_faces[corner]),
            }
        });
        Around {
            edges,
            faces,
            edge_faces,
        }
    }
}

fn clamp_sharpness(sharpness: f32) -> f32 {
    match sharpness {
        sharpness if sharpness >= SHARPNESS_INFINITE => INFINITE,
        sharpness => sharpness,
    }
}

fn crease_sharpness(crease: f32) -> f32 {
    clamp_sharpness(crease * crease * 10.0)
}

fn sharpness(creases: bool, values: Option<&[f32]>, len: usize) -> Vec<f32> {
    match (creases, values) {
        (true, Some(values)) => (0..len)
            .map(|index| crease_sharpness(values.get(index).copied().unwrap_or(0.0)))
            .collect(),
        (true, None) => vec![crease_sharpness(0.0); len],
        (false, _) => vec![0.0; len],
    }
}

fn edge_faces(cage: &Cage) -> Vec<u32> {
    let mut count = vec![0u32; cage.edges.len()];
    for &edge in &cage.corner_edges {
        count[edge as usize] += 1;
    }
    count
}

pub(super) fn vertex_cage(mesh: &Polymesh, creases: bool, boundary_corners: bool) -> Cage {
    let mut cage = Cage {
        points: mesh.positions.len(),
        faces: mesh.face_offsets.clone(),
        corners: mesh.corner_verts.clone(),
        edges: mesh.edges.clone(),
        corner_edges: mesh.corner_edges.clone(),
        edge_sharpness: sharpness(creases, mesh.edge_creases(), mesh.edges.len()),
        point_sharpness: sharpness(creases, mesh.vertex_creases(), mesh.positions.len()),
    };
    let mut count = vec![0u8; cage.points];
    let mut first = vec![[0u32; 2]; cage.points];
    for (edge, &[a, b]) in cage.edges.iter().enumerate() {
        for point in [a as usize, b as usize] {
            if count[point] < 2 {
                first[point][count[point] as usize] = edge as u32;
            }
            count[point] = count[point].saturating_add(1);
        }
    }
    for vertex in 0..cage.points {
        if count[vertex] == 2 && cage.point_sharpness[vertex].is_finite() {
            let [a, b] = first[vertex];
            let added = cage.edge_sharpness[a as usize].min(cage.edge_sharpness[b as usize]);
            cage.point_sharpness[vertex] = clamp_sharpness(cage.point_sharpness[vertex] + added);
        }
    }
    sharpen_boundaries(&mut cage, boundary_corners);
    cage
}

fn sharpen_boundaries(cage: &mut Cage, corners: bool) {
    let users = edge_faces(cage);
    for (edge, &count) in users.iter().enumerate() {
        if count != 2 {
            cage.edge_sharpness[edge] = INFINITE;
        }
    }
    let mut faces_at = vec![0u32; cage.points];
    for &point in &cage.corners {
        faces_at[point as usize] += 1;
    }
    let mut boundary_at = vec![0u32; cage.points];
    for (edge, &[a, b]) in cage.edges.iter().enumerate() {
        if users[edge] != 2 {
            boundary_at[a as usize] += 1;
            boundary_at[b as usize] += 1;
        }
    }
    for point in 0..cage.points {
        let corner = boundary_at[point] > 0 && faces_at[point] == 1;
        if (corners && corner) || (boundary_at[point] > 0 && boundary_at[point] != 2) {
            cage.point_sharpness[point] = INFINITE;
        }
    }
}

pub(super) struct UvCage {
    pub(super) cage: Cage,
    pub(super) values: Vec<Vec2>,
    pub(super) linear: bool,
}

pub(super) fn uv_cage(
    mesh: &Polymesh,
    uvs: &[Vec2],
    cage: &Cage,
    uv_smooth: UvSmooth,
    boundary_corners: bool,
) -> UvCage {
    let mut ids: FxHashMap<(u32, [u32; 2]), u32> = FxHashMap::default();
    let mut values = Vec::new();
    let mut points = Vec::with_capacity(mesh.corner_count());
    let mut owner = Vec::new();
    for corner in 0..mesh.corner_count() {
        let uv = uvs[corner];
        let key = (mesh.corner_verts[corner], uv.to_array().map(f32::to_bits));
        let id = *ids.entry(key).or_insert_with(|| {
            values.push(uv);
            owner.push(mesh.corner_verts[corner]);
            (values.len() - 1) as u32
        });
        points.push(id);
    }
    let mut edge_ids: FxHashMap<[u32; 2], u32> = FxHashMap::default();
    let mut edges = Vec::new();
    let mut edge_sharpness = Vec::new();
    let mut corner_edges = Vec::with_capacity(points.len());
    for face in 0..mesh.face_count() {
        let range = mesh.face(face);
        for corner in range.clone() {
            let next = mesh.next_corner(face, corner);
            let (a, b) = (points[corner], points[next]);
            let key = match a < b {
                true => [a, b],
                false => [b, a],
            };
            let id = *edge_ids.entry(key).or_insert_with(|| {
                edges.push(key);
                edge_sharpness.push(cage.edge_sharpness[mesh.corner_edges[corner] as usize]);
                (edges.len() - 1) as u32
            });
            corner_edges.push(id);
        }
    }
    let point_sharpness = owner
        .iter()
        .map(|&vertex| cage.point_sharpness[vertex as usize])
        .collect();
    let mut fvar = Cage {
        points: values.len(),
        faces: mesh.face_offsets.clone(),
        corners: points,
        edges,
        corner_edges,
        edge_sharpness,
        point_sharpness,
    };
    let linear = uv_smooth == UvSmooth::None;
    let users = edge_faces(&fvar);
    for (edge, &count) in users.iter().enumerate() {
        if count != 2 {
            fvar.edge_sharpness[edge] = INFINITE;
        }
    }
    match uv_smooth {
        UvSmooth::All => sharpen_boundaries(&mut fvar, boundary_corners),
        UvSmooth::PreserveBoundaries => {
            for (edge, &count) in users.iter().enumerate() {
                if count != 2 {
                    let [a, b] = fvar.edges[edge];
                    fvar.point_sharpness[a as usize] = INFINITE;
                    fvar.point_sharpness[b as usize] = INFINITE;
                }
            }
        }
        UvSmooth::None
        | UvSmooth::PreserveCorners
        | UvSmooth::PreserveCornersAndJunctions
        | UvSmooth::PreserveCornersJunctionsAndConcave => sharpen_boundaries(&mut fvar, true),
    }
    UvCage {
        cage: fvar,
        values,
        linear,
    }
}
