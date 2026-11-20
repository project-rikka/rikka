use glam::{Vec2, Vec3};

#[must_use]
pub fn normal(points: impl Iterator<Item = Vec3> + Clone) -> Option<Vec3> {
    let mut previous = points.clone().last()?;
    let mut normal = Vec3::ZERO;
    for current in points {
        normal.x += (previous.y - current.y) * (previous.z + current.z);
        normal.y += (previous.z - current.z) * (previous.x + current.x);
        normal.z += (previous.x - current.x) * (previous.y + current.y);
        previous = current;
    }
    match normal.length() {
        length if length > 0.0 && length.is_finite() => Some(normal / length),
        _ => None,
    }
}

#[must_use]
pub fn project(points: impl Iterator<Item = Vec3>, normal: Vec3) -> Vec<Vec2> {
    let (first, second) = basis(-normal);
    points
        .map(|point| Vec2::new(first.dot(point), second.dot(point)))
        .collect()
}

#[must_use]
pub fn triangulate(points: &[Vec3]) -> Vec<[u32; 3]> {
    match points.len() {
        0..=2 => Vec::new(),
        3 => vec![[0, 1, 2]],
        _ => {
            let facing = normal(points.iter().copied()).unwrap_or(Vec3::Z);
            triangulate_flat(&project(points.iter().copied(), facing))
        }
    }
}

#[must_use]
pub fn basis(normal: Vec3) -> (Vec3, Vec3) {
    let squared = normal.x * normal.x + normal.y * normal.y;
    if squared > f32::EPSILON {
        let scale = 1.0 / squared.sqrt();
        let first = Vec3::new(normal.y * scale, -normal.x * scale, 0.0);
        let second = Vec3::new(
            -normal.z * first.y,
            normal.z * first.x,
            normal.x * first.y - normal.y * first.x,
        );
        (first, second)
    } else {
        let sign = if normal.z < 0.0 { -1.0 } else { 1.0 };
        (Vec3::new(sign, 0.0, 0.0), Vec3::Y)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sign {
    Concave,
    Tangential,
    Convex,
}

fn sign(a: Vec2, b: Vec2, c: Vec2) -> Sign {
    let along_b = b - c;
    let along_a = a - c;
    let area = along_b.x * along_a.y - along_a.x * along_b.y;
    if area > 0.0 {
        Sign::Convex
    } else if area == 0.0 {
        Sign::Tangential
    } else {
        Sign::Concave
    }
}

struct Ring {
    next: Vec<usize>,
    previous: Vec<usize>,
    index: Vec<u32>,
    signs: Vec<Sign>,
}

#[must_use]
pub fn triangulate_flat(coords: &[Vec2]) -> Vec<[u32; 3]> {
    let count = coords.len();
    if count < 3 {
        return Vec::new();
    }
    let mut cross = 0.0;
    let mut previous = coords[count - 1];
    for &current in coords {
        cross += (previous.x - current.x) * (current.y + previous.y);
        previous = current;
    }
    let forward = cross <= 0.0;
    let mut ring = Ring {
        next: (0..count).map(|slot| (slot + 1) % count).collect(),
        previous: (0..count).map(|slot| (slot + count - 1) % count).collect(),
        index: (0..count)
            .map(|slot| match forward {
                true => slot as u32,
                false => (count - 1 - slot) as u32,
            })
            .collect(),
        signs: vec![Sign::Convex; count],
    };
    let point = |ring: &Ring, slot: usize| coords[ring.index[slot] as usize];
    let measure = |ring: &Ring, slot: usize| {
        sign(
            point(ring, ring.previous[slot]),
            point(ring, slot),
            point(ring, ring.next[slot]),
        )
    };
    let mut concave = 0;
    for slot in 0..count {
        ring.signs[slot] = measure(&ring, slot);
        if ring.signs[slot] != Sign::Convex {
            concave += 1;
        }
    }

    let mut triangles = Vec::with_capacity(count - 2);
    let mut head = 0;
    let mut remaining = count;
    let mut ear_start = head;
    let mut reverse = false;
    while remaining > 3 {
        let ear = find_ear(&ring, coords, ear_start, reverse, remaining, concave);
        if ring.signs[ear] != Sign::Convex {
            concave -= 1;
        }
        let previous = ring.previous[ear];
        let next = ring.next[ear];
        triangles.push([ring.index[previous], ring.index[ear], ring.index[next]]);
        ring.next[previous] = next;
        ring.previous[next] = previous;
        if head == ear {
            head = next;
        }
        remaining -= 1;

        for neighbour in [previous, next] {
            if ring.signs[neighbour] != Sign::Convex {
                ring.signs[neighbour] = measure(&ring, neighbour);
                if ring.signs[neighbour] == Sign::Convex {
                    concave -= 1;
                }
            }
        }
        ear_start = match reverse {
            true => ring.previous[previous],
            false => ring.next[next],
        };
        if ring.signs[ear_start] != Sign::Convex {
            ear_start = match reverse {
                true => ring.previous[ear_start],
                false => ring.next[ear_start],
            };
            reverse = !reverse;
        }
    }
    if remaining == 3 {
        let first = head;
        let second = ring.next[first];
        let third = ring.next[second];
        triangles.push([ring.index[first], ring.index[second], ring.index[third]]);
    }
    triangles
}

fn find_ear(
    ring: &Ring,
    coords: &[Vec2],
    start: usize,
    reverse: bool,
    remaining: usize,
    concave: usize,
) -> usize {
    for accept in [Sign::Convex, Sign::Tangential] {
        let mut ear = start;
        for _ in 0..remaining {
            if is_ear(ring, coords, ear, accept, concave) {
                return ear;
            }
            ear = match reverse {
                true => ring.previous[ear],
                false => ring.next[ear],
            };
        }
    }
    let mut ear = start;
    for _ in 0..remaining {
        if ring.signs[ear] != Sign::Concave {
            return ear;
        }
        ear = ring.next[ear];
    }
    ear
}

fn is_ear(ring: &Ring, coords: &[Vec2], tip: usize, accept: Sign, concave: usize) -> bool {
    if concave == 0 {
        return true;
    }
    if ring.signs[tip] != accept {
        return false;
    }
    let point = |slot: usize| coords[ring.index[slot] as usize];
    let first = point(ring.previous[tip]);
    let second = point(tip);
    let third = point(ring.next[tip]);
    let mut checked = 0;
    let mut current = ring.next[ring.next[tip]];
    while current != ring.previous[tip] {
        if ring.signs[current] != Sign::Convex {
            let candidate = point(current);
            if sign(third, first, candidate) != Sign::Concave
                && sign(first, second, candidate) != Sign::Concave
                && sign(second, third, candidate) != Sign::Concave
            {
                return false;
            }
            checked += 1;
            if checked == concave {
                break;
            }
        }
        current = ring.next[current];
    }
    true
}
