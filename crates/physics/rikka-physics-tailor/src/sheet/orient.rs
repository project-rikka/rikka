use std::collections::HashMap;

use glam::Vec3;

use crate::geometry::Shell;

const NEAR_BODY: f32 = 0.3;

pub(crate) fn outward(body: &Shell, point: Vec3, pins: &[Vec3]) -> Vec3 {
    if let Some(signed) = body.signed(point, NEAR_BODY) {
        return signed.outward;
    }
    let axis = if pins.is_empty() {
        point
    } else {
        pins.iter().copied().sum::<Vec3>() / pins.len() as f32
    };
    let away = point - axis;
    Vec3::new(away.x, 0.0, away.z).normalize_or(Vec3::Y)
}

pub(crate) fn orient(positions: &[Vec3], triangles: &mut [[u32; 3]], body: &Shell, pins: &[Vec3]) {
    let mut sides: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (index, triangle) in triangles.iter().enumerate() {
        for side in 0..3 {
            let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
            sides
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(index as u32);
        }
    }
    let directed = |triangle: [u32; 3], a: u32, b: u32| {
        (0..3).any(|side| triangle[side] == a && triangle[(side + 1) % 3] == b)
    };
    let mut visited = vec![false; triangles.len()];
    for start in 0..triangles.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut patch = vec![start as u32];
        let mut cursor = 0;
        while cursor < patch.len() {
            let current = triangles[patch[cursor] as usize];
            cursor += 1;
            for side in 0..3 {
                let (a, b) = (current[side], current[(side + 1) % 3]);
                let Some(shared) = sides.get(&(a.min(b), a.max(b))) else {
                    continue;
                };
                if shared.len() != 2 {
                    continue;
                }
                for &next in shared {
                    if visited[next as usize] {
                        continue;
                    }
                    visited[next as usize] = true;
                    if directed(triangles[next as usize], a, b) {
                        triangles[next as usize].swap(1, 2);
                    }
                    patch.push(next);
                }
            }
        }
        let alignment: f32 = patch
            .iter()
            .map(|index| {
                let [a, b, c] = triangles[*index as usize].map(|corner| positions[corner as usize]);
                let centre = (a + b + c) / 3.0;
                (b - a).cross(c - a).dot(outward(body, centre, pins))
            })
            .sum();
        if alignment < 0.0 {
            for index in &patch {
                triangles[*index as usize].swap(1, 2);
            }
        }
    }
}
