use glam::{DVec2, DVec3};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::arrangement::{Context, Triangle};
use crate::cdt::{Cdt, CdtOutput, EdgeOrigin};
use crate::exact::projection;
use crate::parallel::map_chunks;

const FACES_PER_TASK: usize = 4096;

type Triangles = SmallVec<[Triangle; 2]>;

pub(crate) fn triangulate_faces(context: &Context) -> Vec<Triangle> {
    let threads = context.settings.threads;
    map_chunks(context.faces.len(), threads, FACES_PER_TASK, |range| {
        range
            .flat_map(|face| triangulate_face(context, face as u32))
            .collect()
    })
}

pub(crate) fn is_degenerate(context: &Context, verts: [u32; 3]) -> bool {
    let [a, b, c] = verts;
    if a == b || b == c || c == a {
        return true;
    }
    let space = context.space();
    let (pa, pb, pc) = (context.point(a), context.point(b), context.point(c));
    (0..3).all(|axis| space.orient2d(axis, pa, pb, pc) == 0)
}

fn triangulate_face(context: &Context, face: u32) -> Triangles {
    let source = &context.faces[face as usize];
    let all_verts = &context.face_verts[face as usize];
    let count = all_verts.len();
    let mut verts: SmallVec<[u32; 8]> = SmallVec::new();
    let mut edges: SmallVec<[Option<u32>; 8]> = SmallVec::new();
    for position in 0..count {
        if all_verts[position] != all_verts[(position + 1) % count] {
            verts.push(all_verts[position]);
            edges.push(source.edges[position]);
        }
    }
    let make = |corners: [usize; 3], edge_slots: [Option<usize>; 3]| Triangle {
        verts: corners.map(|corner| verts[corner]),
        face,
        edges: edge_slots.map(|slot| slot.and_then(|position| edges[position])),
        intersect: [false; 3],
        flipped: false,
    };
    let mut result: Triangles = SmallVec::new();
    match verts.len() {
        0..=2 => {}
        3 => result.push(make([0, 1, 2], [Some(0), Some(1), Some(2)])),
        4 => {
            let positions: [DVec3; 4] = std::array::from_fn(|i| context.point(verts[i]).approx);
            let d12 = positions[1] - positions[0];
            let d13 = positions[2] - positions[0];
            let d14 = positions[3] - positions[0];
            let flip = d12.cross(d13).dot(d14.cross(d13)) > 0.0;
            let quad = match flip {
                true => [
                    make([0, 1, 3], [Some(0), None, Some(3)]),
                    make([1, 2, 3], [Some(1), Some(2), None]),
                ],
                false => [
                    make([0, 1, 2], [Some(0), Some(1), None]),
                    make([0, 2, 3], [None, Some(2), Some(3)]),
                ],
            };
            match quad
                .iter()
                .any(|triangle| is_degenerate(context, triangle.verts))
            {
                true => result = triangulate_polygon(context, face, &verts, &edges),
                false => result.extend(quad),
            }
        }
        _ => result = triangulate_polygon(context, face, &verts, &edges),
    }
    result.retain(|triangle| !is_degenerate(context, triangle.verts));
    result
}

fn newell(positions: &[DVec3]) -> DVec3 {
    let mut normal = DVec3::ZERO;
    for i in 0..positions.len() {
        let (a, b) = (positions[i], positions[(i + 1) % positions.len()]);
        normal += DVec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    normal
}

fn dominant_axis(normal: DVec3) -> usize {
    let magnitude = normal.abs();
    match (
        magnitude.x >= magnitude.y,
        magnitude.y >= magnitude.z,
        magnitude.x >= magnitude.z,
    ) {
        (true, _, true) => 0,
        (false, true, _) => 1,
        _ => 2,
    }
}

fn triangulate_polygon(
    context: &Context,
    face: u32,
    verts: &[u32],
    edges: &[Option<u32>],
) -> Triangles {
    let positions: Vec<DVec3> = verts.iter().map(|&v| context.point(v).approx).collect();
    let normal = newell(&positions);
    let axis = dominant_axis(normal);
    let (u, v) = projection(axis);
    let coords: Vec<DVec2> = positions.iter().map(|p| DVec2::new(p[u], p[v])).collect();
    let reverse = normal[axis] < 0.0;
    let polygon: Vec<u32> = (0..verts.len() as u32).collect();
    let mut result: Triangles = SmallVec::new();
    let finish =
        |corners: [usize; 3], edge_positions: [Option<usize>; 3], reorient: bool| -> Triangle {
            let triangle = Triangle {
                verts: corners.map(|corner| verts[corner]),
                face,
                edges: edge_positions.map(|slot| slot.and_then(|position| edges[position])),
                intersect: [false; 3],
                flipped: false,
            };
            match reorient {
                true => Triangle {
                    flipped: false,
                    ..triangle.flip()
                },
                false => triangle,
            }
        };
    let cdt = Cdt::new(coords.clone())
        .faces([polygon])
        .triangulate(CdtOutput::Inside);
    if let Ok(output) = cdt
        && output
            .vert_origins
            .iter()
            .all(|origins| !origins.is_empty())
    {
        let mut edge_lookup: FxHashMap<(u32, u32), usize> = FxHashMap::default();
        for (index, edge) in output.edges.iter().enumerate() {
            edge_lookup.insert((edge[0].min(edge[1]), edge[0].max(edge[1])), index);
        }
        for triangle in &output.faces {
            let corners: [usize; 3] =
                std::array::from_fn(|i| output.vert_origins[triangle[i] as usize][0] as usize);
            let edge_positions: [Option<usize>; 3] = std::array::from_fn(|i| {
                let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
                let index = edge_lookup.get(&(a.min(b), a.max(b)))?;
                output.edge_origins[*index]
                    .iter()
                    .find_map(|origin| match *origin {
                        EdgeOrigin::FaceEdge { position, .. } => Some(position as usize),
                        EdgeOrigin::Edge(_) => None,
                    })
            });
            result.push(finish(corners, edge_positions, reverse));
        }
        return result;
    }
    for corners in ear_clip(&coords) {
        let edge_positions: [Option<usize>; 3] = std::array::from_fn(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % 3]);
            ((a + 1) % verts.len() == b).then_some(a)
        });
        result.push(finish(corners, edge_positions, false));
    }
    result
}

fn ear_clip(coords: &[DVec2]) -> Vec<[usize; 3]> {
    let area: f64 = (0..coords.len())
        .map(|i| coords[i].perp_dot(coords[(i + 1) % coords.len()]))
        .sum();
    let sign = if area < 0.0 { -1.0 } else { 1.0 };
    let mut remaining: Vec<usize> = (0..coords.len()).collect();
    let mut triangles = Vec::with_capacity(coords.len().saturating_sub(2));
    while remaining.len() > 3 {
        let count = remaining.len();
        let ear = (0..count).find(|&i| {
            let (a, b, c) = (
                remaining[(i + count - 1) % count],
                remaining[i],
                remaining[(i + 1) % count],
            );
            let turn = (coords[b] - coords[a]).perp_dot(coords[c] - coords[b]) * sign;
            turn > 0.0
                && remaining.iter().all(|&p| {
                    p == a
                        || p == b
                        || p == c
                        || !inside_triangle(coords[p], coords[a], coords[b], coords[c], sign)
                })
        });
        let i = ear.unwrap_or(0);
        let (a, b, c) = (
            remaining[(i + count - 1) % count],
            remaining[i],
            remaining[(i + 1) % count],
        );
        triangles.push([a, b, c]);
        remaining.remove(i);
    }
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
    }
    triangles
}

fn inside_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2, sign: f64) -> bool {
    let d1 = (b - a).perp_dot(p - a) * sign;
    let d2 = (c - b).perp_dot(p - b) * sign;
    let d3 = (a - c).perp_dot(p - c) * sign;
    d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
}
