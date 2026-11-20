use rikka_physics_math::Vec3;

use crate::triangulated::triangle::{EdgeFlags, signed_volume};

const COS_FLAT: f32 = 0.9962;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeUse {
    key: u64,
    triangle: u32,
    edge: u8,
}

#[inline]
fn undirected(a: u32, b: u32) -> u64 {
    u64::from(a.min(b)) << 32 | u64::from(a.max(b))
}

pub(crate) fn identify_edges(vertices: &[Vec3], triangles: &[[u32; 3]]) -> Vec<u8> {
    let mut uses: Vec<EdgeUse> = triangles
        .iter()
        .enumerate()
        .flat_map(|(triangle, indices)| {
            (0..3u8).map(move |edge| EdgeUse {
                key: undirected(indices[edge as usize], indices[(edge as usize + 1) % 3]),
                triangle: triangle as u32,
                edge,
            })
        })
        .collect();
    uses.sort_unstable();
    let normals: Vec<Vec3> = triangles
        .iter()
        .map(|indices| {
            let [a, b, c] = indices.map(|vertex| vertices[vertex as usize]);
            (b - a).cross(c - a).normalize_or_zero()
        })
        .collect();
    let mut flags = vec![EdgeFlags::NONE; triangles.len()];
    for group in uses.chunk_by(|a, b| a.key == b.key) {
        let [first, second] = group else {
            continue;
        };
        let [a, b, c] = triangles[first.triangle as usize].map(|vertex| vertices[vertex as usize]);
        let other = triangles[second.triangle as usize];
        let opposite = vertices[other[(second.edge as usize + 2) % 3] as usize];
        let volume = signed_volume(a, b, c, opposite);
        let flat =
            normals[first.triangle as usize].dot(normals[second.triangle as usize]) > COS_FLAT;
        let mut mark = |flag: fn(usize) -> EdgeFlags| {
            flags[first.triangle as usize] |= flag(first.edge as usize);
            flags[second.triangle as usize] |= flag(second.edge as usize);
        };
        if volume > 0.0 || flat {
            mark(EdgeFlags::concave);
        }
        if volume < 0.0 || flat {
            mark(EdgeFlags::inverse_concave);
        }
    }
    flags.into_iter().map(|flags| flags.0).collect()
}

pub(crate) fn is_closed(triangles: &[[u32; 3]]) -> bool {
    let mut directed: Vec<(u32, u32)> = triangles
        .iter()
        .flat_map(|indices| (0..3).map(move |edge| (indices[edge], indices[(edge + 1) % 3])))
        .collect();
    directed.sort_unstable();
    if directed.windows(2).any(|pair| pair[0] == pair[1]) {
        return false;
    }
    directed
        .iter()
        .all(|&(from, to)| directed.binary_search(&(to, from)).is_ok())
}
