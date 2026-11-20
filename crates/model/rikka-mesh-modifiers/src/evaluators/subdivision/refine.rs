use super::cage::{Around, Cage, NO_EDGE};
use super::stencil::Stencil;
use crate::parallel;

const GRAIN: usize = 1 << 13;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Rule {
    Smooth,
    Crease(u32, u32),
    Corner,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Extent {
    Full,
    Around(usize),
}

pub(super) struct Refined {
    pub(super) cage: Cage,
    pub(super) smooth: Stencil,
}

fn other_end(edge: [u32; 2], point: u32) -> u32 {
    match edge[0] == point {
        true => edge[1],
        false => edge[0],
    }
}

pub(super) fn rule(cage: &Cage, around: &Around, point: usize, decrement: f32) -> Rule {
    match classify(cage, around, point, decrement) {
        Class::Smooth => Rule::Smooth,
        Class::Crease(first, second) => Rule::Crease(
            other_end(cage.edges[first as usize], point as u32),
            other_end(cage.edges[second as usize], point as u32),
        ),
        Class::Corner => Rule::Corner,
    }
}

enum Class {
    Smooth,
    Crease(u32, u32),
    Corner,
}

fn classify(cage: &Cage, around: &Around, point: usize, decrement: f32) -> Class {
    if cage.point_sharpness[point] - decrement > 0.0 {
        return Class::Corner;
    }
    let mut creased = [0u32; 2];
    let mut count = 0usize;
    for &edge in around.edges.get(point) {
        if cage.edge_sharpness[edge as usize] - decrement > 0.0 {
            if count < 2 {
                creased[count] = edge;
            }
            count += 1;
        }
    }
    match count {
        0 | 1 => Class::Smooth,
        2 => Class::Crease(creased[0], creased[1]),
        _ => Class::Corner,
    }
}

fn face_center(cage: &Cage, face: u32, scale: f32, out: &mut Vec<(u32, f32)>) {
    let range = cage.face(face as usize);
    let share = 1.0 / range.len() as f32;
    for corner in range {
        out.push((cage.corners[corner], share * scale));
    }
}

fn face_center_plain(cage: &Cage, face: u32, out: &mut Vec<(u32, f32)>) {
    let range = cage.face(face as usize);
    let share = 1.0 / range.len() as f32;
    for corner in range {
        out.push((cage.corners[corner], share));
    }
}

fn mask(cage: &Cage, around: &Around, point: usize, rule: Rule, out: &mut Vec<(u32, f32)>) {
    match rule {
        Rule::Corner => out.push((point as u32, 1.0)),
        Rule::Crease(first, second) => {
            out.extend([(first, 0.125), (point as u32, 0.75), (second, 0.125)]);
        }
        Rule::Smooth => {
            let edges = around.edges.get(point);
            let faces = around.faces.get(point);
            if edges.is_empty() || faces.is_empty() {
                out.push((point as u32, 1.0));
                return;
            }
            let valence = edges.len() as f32;
            let squared = valence * valence;
            out.push((point as u32, (valence - 2.0) / valence));
            for &edge in edges {
                out.push((
                    other_end(cage.edges[edge as usize], point as u32),
                    1.0 / squared,
                ));
            }
            let face_share = valence / faces.len() as f32 / squared;
            for &face in faces {
                face_center(cage, face, face_share, out);
            }
        }
    }
}

fn point_row(cage: &Cage, around: &Around, point: usize, linear: bool, out: &mut Vec<(u32, f32)>) {
    if linear {
        out.push((point as u32, 1.0));
        return;
    }
    let parent = rule(cage, around, point, 0.0);
    let child = rule(cage, around, point, 1.0);
    if parent == child {
        mask(cage, around, point, parent, out);
        return;
    }
    let transitions = std::iter::once(cage.point_sharpness[point])
        .chain(
            around
                .edges
                .get(point)
                .iter()
                .map(|&edge| cage.edge_sharpness[edge as usize]),
        )
        .filter(|&sharpness| sharpness > 0.0 && sharpness - 1.0 <= 0.0);
    let count = transitions.clone().count();
    let fraction = match count {
        0 => 1.0,
        _ => transitions.sum::<f32>() / count as f32,
    };
    let start = out.len();
    mask(cage, around, point, parent, out);
    for entry in &mut out[start..] {
        entry.1 *= fraction;
    }
    let middle = out.len();
    mask(cage, around, point, child, out);
    for entry in &mut out[middle..] {
        entry.1 *= 1.0 - fraction;
    }
}

fn edge_row(cage: &Cage, around: &Around, edge: usize, linear: bool, out: &mut Vec<(u32, f32)>) {
    let [a, b] = cage.edges[edge];
    let faces = around.edge_faces.get(edge);
    let sharpness = cage.edge_sharpness[edge];
    if linear || faces.len() != 2 || sharpness >= 1.0 {
        out.extend([(a, 0.5), (b, 0.5)]);
        return;
    }
    let start = out.len();
    out.extend([(a, 0.25), (b, 0.25)]);
    for &face in faces {
        face_center(cage, face, 0.25, out);
    }
    if sharpness > 0.0 {
        for entry in &mut out[start..] {
            entry.1 *= 1.0 - sharpness;
        }
        out.push((a, 0.5 * sharpness));
        out.push((b, 0.5 * sharpness));
    }
}

pub(super) fn ptex_ordered<T>(child: [T; 4], size: usize, slot: usize) -> [T; 4] {
    let mut child = child;
    if size == 4 {
        child.rotate_right(slot);
    }
    child
}

pub(super) fn refine(cage: &Cage, linear: bool, extent: Extent) -> Refined {
    refine_with(cage, &Around::new(cage), linear, extent)
}

pub(super) fn refine_with(cage: &Cage, around: &Around, linear: bool, extent: Extent) -> Refined {
    let smooth = smooth_stencil(cage, around, linear, extent);
    let cage = match extent {
        Extent::Full => full_child(cage),
        Extent::Around(originals) => restricted_child(cage, originals),
    };
    Refined { cage, smooth }
}

pub(super) fn smooth_stencil(
    cage: &Cage,
    around: &Around,
    linear: bool,
    extent: Extent,
) -> Stencil {
    let kept_points = match extent {
        Extent::Full => cage.points,
        Extent::Around(originals) => originals,
    };
    let edge_count = cage.edges.len();
    let face_count = cage.face_count();
    let rows = kept_points + edge_count + face_count;
    Stencil::build(rows, |row, out| {
        if row < kept_points {
            point_row(cage, around, row, linear, out);
        } else if row < kept_points + edge_count {
            edge_row(cage, around, row - kept_points, linear, out);
        } else {
            face_center_plain(cage, (row - kept_points - edge_count) as u32, out);
        }
    })
}

fn full_child(cage: &Cage) -> Cage {
    let points = cage.points;
    let edge_count = cage.edges.len();
    let face_count = cage.face_count();
    let corner_count = cage.corners.len();
    let edge_point = |edge: u32| (points as u32) + edge;
    let face_point = |face: usize| (points + edge_count + face) as u32;
    let half = |edge: u32, point: u32| -> u32 {
        match cage.edges[edge as usize][0] == point {
            true => edge * 2,
            false => edge * 2 + 1,
        }
    };
    let inner = |corner: usize| (edge_count * 2 + corner) as u32;
    let mut corners = vec![0u32; corner_count * 4];
    let mut corner_edges = vec![0u32; corner_count * 4];
    let ranges = parallel::ranges(face_count, GRAIN / 4);
    let corner_ranges: Vec<std::ops::Range<usize>> = ranges
        .iter()
        .map(|range| cage.faces[range.start] as usize * 4..cage.faces[range.end] as usize * 4)
        .collect();
    let face_ranges = &ranges;
    let fill = |values: &mut [u32],
                pick: &(dyn Fn(usize, usize, usize, usize) -> [u32; 4] + Sync)| {
        parallel::split_mut(values, &corner_ranges, |index, piece| {
            let base = corner_ranges[index].start;
            for face in face_ranges[index].clone() {
                let start = cage.faces[face] as usize;
                let end = cage.faces[face + 1] as usize;
                let size = end - start;
                for slot in 0..size {
                    let corner = start + slot;
                    let previous = start + (slot + size - 1) % size;
                    let child = ptex_ordered(pick(face, corner, previous, size), size, slot);
                    piece[corner * 4 - base..corner * 4 - base + 4].copy_from_slice(&child);
                }
            }
        });
    };
    fill(&mut corners, &|face, corner, previous, _| {
        [
            cage.corners[corner],
            edge_point(cage.corner_edges[corner]),
            face_point(face),
            edge_point(cage.corner_edges[previous]),
        ]
    });
    fill(&mut corner_edges, &|_, corner, previous, _| {
        let point = cage.corners[corner];
        let edge = cage.corner_edges[corner];
        let previous_edge = cage.corner_edges[previous];
        [
            half(edge, point),
            inner(corner),
            inner(previous),
            half(previous_edge, point),
        ]
    });
    let faces: Vec<u32> = (0..=corner_count as u32).map(|face| face * 4).collect();
    let corner_faces = cage.corner_faces();
    let edges: Vec<[u32; 2]> = parallel::collect(edge_count * 2 + corner_count, GRAIN, |index| {
        if index < edge_count * 2 {
            let edge = index / 2;
            let [a, b] = cage.edges[edge];
            match index % 2 {
                0 => [a, edge_point(edge as u32)],
                _ => [edge_point(edge as u32), b],
            }
        } else {
            let corner = index - edge_count * 2;
            [
                edge_point(cage.corner_edges[corner]),
                face_point(corner_faces[corner] as usize),
            ]
        }
    });
    let edge_sharpness: Vec<f32> =
        parallel::collect(edges.len(), GRAIN, |index| match index < edge_count * 2 {
            true => (cage.edge_sharpness[index / 2] - 1.0).max(0.0),
            false => 0.0,
        });
    let mut point_sharpness: Vec<f32> = cage
        .point_sharpness
        .iter()
        .map(|sharpness| (sharpness - 1.0).max(0.0))
        .collect();
    point_sharpness.resize(points + edge_count + face_count, 0.0);
    Cage {
        points: points + edge_count + face_count,
        faces,
        corners,
        edges,
        corner_edges,
        edge_sharpness,
        point_sharpness,
    }
}

fn restricted_child(cage: &Cage, originals: usize) -> Cage {
    let edge_count = cage.edges.len();
    let face_count = cage.face_count();
    let edge_point = |edge: u32| originals as u32 + edge;
    let face_point = |face: usize| (originals + edge_count + face) as u32;
    let mut halves = vec![NO_EDGE; edge_count * 2];
    let mut edges = Vec::with_capacity(edge_count);
    let mut edge_sharpness = Vec::with_capacity(edge_count);
    for (edge, &[a, b]) in cage.edges.iter().enumerate() {
        let sharpness = (cage.edge_sharpness[edge] - 1.0).max(0.0);
        if (a as usize) < originals {
            halves[edge * 2] = edges.len() as u32;
            edges.push([a, edge_point(edge as u32)]);
            edge_sharpness.push(sharpness);
        }
        if (b as usize) < originals {
            halves[edge * 2 + 1] = edges.len() as u32;
            edges.push([edge_point(edge as u32), b]);
            edge_sharpness.push(sharpness);
        }
    }
    let half = |edge: u32, point: u32| -> u32 {
        match edge == NO_EDGE {
            true => NO_EDGE,
            false => match cage.edges[edge as usize][0] == point {
                true => halves[edge as usize * 2],
                false => halves[edge as usize * 2 + 1],
            },
        }
    };
    let mut faces = vec![0u32];
    let mut corners = Vec::new();
    let mut corner_edges = Vec::new();
    for face in 0..face_count {
        let start = cage.faces[face] as usize;
        let size = cage.faces[face + 1] as usize - start;
        for slot in 0..size {
            let corner = start + slot;
            let point = cage.corners[corner];
            if point as usize >= originals {
                continue;
            }
            let previous = start + (slot + size - 1) % size;
            let edge = cage.corner_edges[corner];
            let previous_edge = cage.corner_edges[previous];
            corners.extend(ptex_ordered(
                [
                    point,
                    edge_point(edge),
                    face_point(face),
                    edge_point(previous_edge),
                ],
                size,
                slot,
            ));
            corner_edges.extend(ptex_ordered(
                [
                    half(edge, point),
                    NO_EDGE,
                    NO_EDGE,
                    half(previous_edge, point),
                ],
                size,
                slot,
            ));
            faces.push(corners.len() as u32);
        }
    }
    let mut point_sharpness: Vec<f32> = cage.point_sharpness[..originals]
        .iter()
        .map(|sharpness| (sharpness - 1.0).max(0.0))
        .collect();
    point_sharpness.resize(originals + edge_count + face_count, 0.0);
    Cage {
        points: originals + edge_count + face_count,
        faces,
        corners,
        edges,
        corner_edges,
        edge_sharpness,
        point_sharpness,
    }
}

pub(super) fn limit_stencil(cage: &Cage, around: &Around, points: usize) -> Stencil {
    Stencil::build(points, |point, out| match rule(cage, around, point, 0.0) {
        Rule::Corner => out.push((point as u32, 1.0)),
        Rule::Crease(..) if around.faces.get(point).len() == 1 => out.push((point as u32, 1.0)),
        Rule::Crease(first, second) => out.extend([
            (first, 1.0 / 6.0),
            (point as u32, 4.0 / 6.0),
            (second, 1.0 / 6.0),
        ]),
        Rule::Smooth => {
            let edges = around.edges.get(point);
            let faces = around.faces.get(point);
            let quads = faces
                .iter()
                .all(|&face| cage.faces[face as usize + 1] - cage.faces[face as usize] == 4);
            if edges.is_empty() || faces.len() != edges.len() || !quads {
                out.push((point as u32, 1.0));
                return;
            }
            let valence = edges.len() as f32;
            let total = valence * (valence + 5.0);
            out.push((point as u32, valence * valence / total));
            for &edge in edges {
                out.push((
                    other_end(cage.edges[edge as usize], point as u32),
                    4.0 / total,
                ));
            }
            for &face in faces {
                let start = cage.faces[face as usize] as usize;
                let slot = (start..start + 4)
                    .position(|corner| cage.corners[corner] == point as u32)
                    .unwrap_or(0);
                out.push((cage.corners[start + (slot + 2) % 4], 1.0 / total));
            }
        }
    })
}

pub(super) fn limit_after_refine(cage: &Cage, around: &Around, originals: usize) -> Stencil {
    let edge_point = |edge: u32| originals as u32 + edge;
    let face_point = |face: u32| (originals + cage.edges.len()) as u32 + face;
    Stencil::build(originals, |point, out| {
        let faces = around.faces.get(point);
        match classify(cage, around, point, 1.0) {
            Class::Corner => out.push((point as u32, 1.0)),
            Class::Crease(..) if faces.len() == 1 => out.push((point as u32, 1.0)),
            Class::Crease(first, second) => out.extend([
                (edge_point(first), 1.0 / 6.0),
                (point as u32, 4.0 / 6.0),
                (edge_point(second), 1.0 / 6.0),
            ]),
            Class::Smooth => {
                let edges = around.edges.get(point);
                if edges.is_empty() || faces.len() != edges.len() {
                    out.push((point as u32, 1.0));
                    return;
                }
                let valence = edges.len() as f32;
                let total = valence * (valence + 5.0);
                out.push((point as u32, valence * valence / total));
                for &edge in edges {
                    out.push((edge_point(edge), 4.0 / total));
                }
                for &face in faces {
                    out.push((face_point(face), 1.0 / total));
                }
            }
        }
    })
}

pub(super) fn has_smooth_corners(cage: &Cage, around: &Around) -> bool {
    (0..cage.points).any(|point| {
        around.faces.get(point).len() == 1
            && matches!(rule(cage, around, point, 0.0), Rule::Crease(..))
    })
}
