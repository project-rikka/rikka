use glam::DVec3;
use rustc_hash::FxHashMap;

use crate::arrangement::{Arrangement, Context, OutputFace, OutputVertex, Triangle};
use crate::merge::{MergeRules, merge_triangles};
use crate::parallel::map_chunks;

const FACES_PER_TASK: usize = 2048;
const IN_LINE_TOLERANCE: f64 = 1.0e-4;

#[derive(Clone, Copy, Default)]
struct EdgeInfo {
    orig: Option<u32>,
    intersect: bool,
}

fn normal(context: &Context, triangle: &Triangle) -> DVec3 {
    let [a, b, c] = triangle.verts.map(|v| context.point(v).approx);
    (b - a).cross(c - a)
}

fn cyclic_equal(a: &[u32], b: &[u32]) -> bool {
    a.len() == b.len()
        && (0..a.len()).any(|shift| (0..a.len()).all(|i| a[(i + shift) % a.len()] == b[i]))
}

fn common_edge(first: &Triangle, second: &Triangle) -> Option<(usize, usize)> {
    for i in 0..3 {
        for j in 0..3 {
            if first.verts[(i + 1) % 3] == second.verts[j]
                && first.verts[i] == second.verts[(j + 1) % 3]
            {
                return Some((i, j));
            }
        }
    }
    None
}

fn unchanged_quad(context: &Context, face: u32, tris: &[&Triangle]) -> Option<OutputFace> {
    let [first, second] = [tris[0], tris[1]];
    if normal(context, first).dot(normal(context, second)) <= 0.0 {
        return None;
    }
    let input = &context.face_verts[face as usize];
    if input.len() != 4 {
        return None;
    }
    let (i, j) = common_edge(first, second)?;
    if first.edges[i].is_some() {
        return None;
    }
    let candidate = [
        first.verts[(i + 1) % 3],
        first.verts[(i + 2) % 3],
        first.verts[i],
        second.verts[(j + 2) % 3],
    ];
    if !cyclic_equal(&candidate, input) {
        return None;
    }
    Some(OutputFace {
        face,
        verts: input.clone(),
        edges: context.faces[face as usize].edges.clone(),
        intersect: vec![false; 4],
        flipped: false,
    })
}

fn merge_face(context: &Context, face: u32, tris: &[&Triangle]) -> Vec<OutputFace> {
    if tris.len() == 1 {
        let triangle = tris[0];
        return vec![OutputFace {
            face,
            verts: triangle.verts.to_vec(),
            edges: triangle.edges.to_vec(),
            intersect: triangle.intersect.to_vec(),
            flipped: triangle.flipped,
        }];
    }
    if tris.len() == 2
        && let Some(quad) = unchanged_quad(context, face, tris)
    {
        return vec![quad];
    }
    let mut info: FxHashMap<(u32, u32), EdgeInfo> = FxHashMap::default();
    for triangle in tris {
        for i in 0..3 {
            let (a, b) = (triangle.verts[i], triangle.verts[(i + 1) % 3]);
            let entry = info.entry((a.min(b), a.max(b))).or_default();
            if entry.orig.is_none() {
                entry.orig = triangle.edges[i];
            }
            entry.intersect |= triangle.intersect[i];
        }
    }
    let first_normal = normal(context, tris[0]);
    let mut result = Vec::new();
    for direction in [first_normal, -first_normal] {
        let group: Vec<&Triangle> = tris
            .iter()
            .copied()
            .filter(|triangle| normal(context, triangle).dot(direction) > 0.0)
            .collect();
        if group.is_empty() {
            continue;
        }
        let flipped = group[0].flipped;
        let corners: Vec<[u32; 3]> = group.iter().map(|triangle| triangle.verts).collect();
        let rules = MergeRules {
            dissolvable: |a: u32, b: u32| {
                let edge = info[&(a.min(b), a.max(b))];
                edge.orig.is_none() && !edge.intersect
            },
            length_squared: |a: u32, b: u32| {
                context
                    .point(a)
                    .approx
                    .distance_squared(context.point(b).approx)
            },
            allow_repeated_verts: false,
        };
        for polygon in merge_triangles(&corners, rules) {
            let edges: Vec<EdgeInfo> = (0..polygon.len())
                .map(|i| {
                    let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
                    info[&(a.min(b), a.max(b))]
                })
                .collect();
            result.push(OutputFace {
                face,
                verts: polygon,
                edges: edges.iter().map(|edge| edge.orig).collect(),
                intersect: edges.iter().map(|edge| edge.intersect).collect(),
                flipped,
            });
        }
    }
    result
}

fn in_line(a: DVec3, b: DVec3, c: DVec3) -> bool {
    let cosine = (b - a).normalize_or_zero().dot((c - b).normalize_or_zero());
    (cosine - 1.0).abs() < IN_LINE_TOLERANCE
}

fn dissolve_verts(context: &Context, faces: &mut Vec<OutputFace>) {
    let mut neighbors: FxHashMap<u32, Option<(u32, u32)>> = FxHashMap::default();
    for face in faces.iter() {
        let count = face.verts.len();
        for i in 0..count {
            let v = face.verts[i];
            if context.table.is_input(v) {
                continue;
            }
            let pair = (
                face.verts[(i + 1) % count],
                face.verts[(i + count - 1) % count],
            );
            neighbors
                .entry(v)
                .and_modify(|seen| {
                    if let Some((n1, n2)) = *seen
                        && !((pair.0 == n2 && pair.1 == n1) || (pair.0 == n1 && pair.1 == n2))
                    {
                        *seen = None;
                    }
                })
                .or_insert(Some(pair));
        }
    }
    let dissolve: FxHashMap<u32, ()> = neighbors
        .into_iter()
        .filter_map(|(v, pair)| {
            let (n1, n2) = pair?;
            let point = |x: u32| context.point(x).approx;
            in_line(point(n1), point(v), point(n2)).then_some((v, ()))
        })
        .collect();
    if dissolve.is_empty() {
        return;
    }
    for face in faces.iter_mut() {
        if !face.verts.iter().any(|v| dissolve.contains_key(v)) {
            continue;
        }
        let mut verts = Vec::with_capacity(face.verts.len());
        let mut edges = Vec::with_capacity(face.verts.len());
        let mut intersect = Vec::with_capacity(face.verts.len());
        for i in 0..face.verts.len() {
            if !dissolve.contains_key(&face.verts[i]) {
                verts.push(face.verts[i]);
                edges.push(face.edges[i]);
                intersect.push(face.intersect[i]);
            }
        }
        face.verts = verts;
        face.edges = edges;
        face.intersect = intersect;
    }
    faces.retain(|face| face.verts.len() >= 3);
}

pub(crate) fn recover(context: &Context, triangles: &[Triangle], origins: &[u32]) -> Arrangement {
    let mut by_face: Vec<Vec<u32>> = vec![Vec::new(); context.faces.len()];
    for (index, triangle) in triangles.iter().enumerate() {
        by_face[triangle.face as usize].push(index as u32);
    }
    let mut faces: Vec<OutputFace> = map_chunks(
        by_face.len(),
        context.settings.threads,
        FACES_PER_TASK,
        |range| {
            range
                .flat_map(|face| {
                    let tris: Vec<&Triangle> = by_face[face]
                        .iter()
                        .map(|&t| &triangles[t as usize])
                        .collect();
                    match tris.is_empty() {
                        true => Vec::new(),
                        false => merge_face(context, face as u32, &tris),
                    }
                })
                .collect()
        },
    );
    dissolve_verts(context, &mut faces);
    let mut used = vec![false; context.table.len()];
    for face in &faces {
        for &v in &face.verts {
            used[v as usize] = true;
        }
    }
    let mut remap = vec![u32::MAX; context.table.len()];
    let mut verts = Vec::new();
    for (vertex, &is_used) in used.iter().enumerate() {
        if !is_used {
            continue;
        }
        remap[vertex] = verts.len() as u32;
        let point = context.table.point(vertex as u32);
        verts.push(OutputVertex {
            origin: context
                .table
                .is_input(vertex as u32)
                .then(|| origins[vertex]),
            position: point.approx.as_vec3(),
        });
    }
    for face in &mut faces {
        for v in &mut face.verts {
            *v = remap[*v as usize];
        }
    }
    Arrangement { verts, faces }
}
