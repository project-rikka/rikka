use glam::Vec3;
use rustc_hash::FxHashMap;

pub(crate) fn subdivide(
    positions: &mut Vec<Vec3>,
    pinned: &mut Vec<bool>,
    triangles: &mut Vec<[u32; 3]>,
) -> Vec<[u32; 2]> {
    let mut wings: FxHashMap<(u32, u32), Vec<u32>> = FxHashMap::default();
    for triangle in triangles.iter() {
        for side in 0..3 {
            let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
            wings
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push(triangle[(side + 2) % 3]);
        }
    }
    let mut sides: Vec<(u32, u32)> = wings.keys().copied().collect();
    sides.sort_unstable();
    let count = positions.len();
    let mut neighbours: Vec<Vec<u32>> = vec![Vec::new(); count];
    let mut rim: Vec<Vec<u32>> = vec![Vec::new(); count];
    for &(a, b) in &sides {
        neighbours[a as usize].push(b);
        neighbours[b as usize].push(a);
        if wings[&(a, b)].len() == 1 {
            rim[a as usize].push(b);
            rim[b as usize].push(a);
        }
    }
    let point = |vertex: u32| positions[vertex as usize];
    let moved: Vec<Vec3> = (0..count)
        .map(|vertex| {
            let at = positions[vertex];
            match (
                pinned[vertex],
                rim[vertex].as_slice(),
                neighbours[vertex].len(),
            ) {
                (true, _, _) | (false, _, 0) => at,
                (false, [before, after], _) => at * 0.75 + (point(*before) + point(*after)) * 0.125,
                (false, [], valence) => {
                    let n = valence as f32;
                    let share = match valence {
                        3 => 3.0 / 16.0,
                        _ => 3.0 / (8.0 * n),
                    };
                    let around: Vec3 = neighbours[vertex].iter().map(|other| point(*other)).sum();
                    at * (1.0 - n * share) + around * share
                }
                (false, _, _) => at,
            }
        })
        .collect();
    let mut middle: FxHashMap<(u32, u32), u32> = FxHashMap::default();
    let mut added: Vec<Vec3> = Vec::with_capacity(sides.len());
    let mut added_pins: Vec<bool> = Vec::with_capacity(sides.len());
    for &(a, b) in &sides {
        let wing = &wings[&(a, b)];
        let position = match wing.as_slice() {
            [c, d] => (point(a) + point(b)) * 0.375 + (point(*c) + point(*d)) * 0.125,
            _ => (point(a) + point(b)) * 0.5,
        };
        middle.insert((a, b), (count + added.len()) as u32);
        added.push(position);
        added_pins.push(pinned[a as usize] && pinned[b as usize]);
    }
    positions.clear();
    positions.extend(moved);
    positions.extend(added);
    pinned.extend(added_pins);
    let between = |a: u32, b: u32| middle[&(a.min(b), a.max(b))];
    *triangles = triangles
        .iter()
        .flat_map(|&[a, b, c]| {
            let (ab, bc, ca) = (between(a, b), between(b, c), between(c, a));
            [[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]
        })
        .collect();
    sides.into_iter().map(|(a, b)| [a, b]).collect()
}
