use std::collections::HashMap;

use glam::Vec3;

use crate::geometry::{Shell, Union, area, is_proper};
use crate::proxy::UNMAPPED;

use super::cluster::Clustered;
use super::orient::orient;
use super::subdivide::subdivide;

const FOLDED: f32 = 0.5;
const MIN_TRIANGLE_QUALITY: f32 = 0.05;
const HOLE_SIDES: usize = 10;
const HOLE_REACH: f32 = 8.0;
const FLIP_SWEEPS: usize = 24;
const FLIP_MARGIN: f32 = 0.05;
const FLIP_TURN: f32 = 0.7;
const MIN_COMPONENT_TRIANGLES: usize = 4;
const SUBDIVIDED_ABOVE: f32 = 1.75;
const SUBDIVISIONS: usize = 3;
const PINCH_SHIFT: f32 = 0.1;
const PINCH_APART: f32 = 1.0e-5;

pub(crate) struct Envelope {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) pinned: Vec<bool>,
    pub(crate) triangles: Vec<[u32; 3]>,
    pub(crate) index: Vec<u32>,
    pub(crate) blended: Vec<(u32, [u32; 2])>,
}

type Sides = HashMap<(u32, u32), Vec<u32>>;

impl Envelope {
    pub(crate) fn build(
        clustered: Clustered,
        seeds: &[([u32; 3], f32)],
        radius: f32,
        spacing: f32,
        body: &Shell,
    ) -> Envelope {
        let Clustered {
            mut positions,
            mut pinned,
            index,
        } = clustered;
        let candidates = candidates(&positions, &index, seeds);
        let mut triangles = manifold(&positions, candidates);
        let pins: Vec<Vec3> = positions
            .iter()
            .zip(&pinned)
            .filter(|(_, pinned)| **pinned)
            .map(|(position, _)| *position)
            .collect();
        orient(&positions, &mut triangles, body, &pins);
        let mut blended = unpinch(&mut positions, &mut pinned, &mut triangles);
        let pinches = blended.clone();
        fill_holes(&positions, &mut triangles, radius * HOLE_REACH);
        improve(&positions, &mut triangles);
        remove_small_components(&mut triangles);
        let drawn = index.len();
        for _ in 0..SUBDIVISIONS {
            if mean_edge(&positions, &triangles) <= SUBDIVIDED_ABOVE * spacing
                || positions.len() >= drawn
            {
                break;
            }
            let first = positions.len() as u32;
            let sources = subdivide(&mut positions, &mut pinned, &mut triangles);
            blended.extend((first..).zip(sources));
        }
        part_pinches(&mut positions, &triangles, &pinches);
        compact(positions, pinned, triangles, &index, &blended)
    }
}

fn candidates(positions: &[Vec3], index: &[u32], seeds: &[([u32; 3], f32)]) -> Vec<[u32; 3]> {
    let mut support: HashMap<[u32; 3], (f32, [u32; 3])> = HashMap::new();
    for (corners, size) in seeds {
        let mapped = corners.map(|corner| index[corner as usize]);
        if mapped.contains(&u32::MAX)
            || mapped[0] == mapped[1]
            || mapped[1] == mapped[2]
            || mapped[2] == mapped[0]
            || !is_proper(mapped.map(|corner| positions[corner as usize]))
        {
            continue;
        }
        let mut key = mapped;
        key.sort_unstable();
        support.entry(key).or_insert((0.0, mapped)).0 += size;
    }
    let mut ranked: Vec<(f32, f32, [u32; 3], [u32; 3])> = support
        .into_iter()
        .map(|(key, (size, triangle))| {
            let corners = triangle.map(|corner| positions[corner as usize]);
            (size, quality(corners), key, triangle)
        })
        .filter(|(_, quality, _, _)| *quality >= MIN_TRIANGLE_QUALITY)
        .collect();
    ranked.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.total_cmp(&a.1))
            .then(a.2.cmp(&b.2))
    });
    ranked
        .into_iter()
        .map(|(_, _, _, triangle)| triangle)
        .collect()
}

fn manifold(positions: &[Vec3], candidates: Vec<[u32; 3]>) -> Vec<[u32; 3]> {
    let mut sides: Sides = HashMap::new();
    let mut accepted: Vec<[u32; 3]> = Vec::new();
    for triangle in candidates {
        let fits = (0..3).all(|side| {
            let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
            let third = triangle[(side + 2) % 3];
            match sides.get(&key(a, b)) {
                None => true,
                Some(others) if others.len() >= 2 => false,
                Some(others) => others.iter().all(|other| {
                    let other = accepted[*other as usize];
                    let wing = other
                        .iter()
                        .copied()
                        .find(|corner| *corner != a && *corner != b)
                        .unwrap_or(a);
                    !folded(positions, [a, b], wing, third)
                }),
            }
        });
        if !fits {
            continue;
        }
        let at = accepted.len() as u32;
        for side in 0..3 {
            sides
                .entry(key(triangle[side], triangle[(side + 1) % 3]))
                .or_default()
                .push(at);
        }
        accepted.push(triangle);
    }
    accepted
}

fn folded(positions: &[Vec3], hinge: [u32; 2], wing: u32, other: u32) -> bool {
    let [a, b] = hinge.map(|end| positions[end as usize]);
    let along = (b - a).normalize_or_zero();
    let across = |end: u32| {
        let offset = positions[end as usize] - a;
        (offset - along * offset.dot(along)).normalize_or_zero()
    };
    across(wing).dot(across(other)) > FOLDED
}

fn unpinch(
    positions: &mut Vec<Vec3>,
    pinned: &mut Vec<bool>,
    triangles: &mut [[u32; 3]],
) -> Vec<(u32, [u32; 2])> {
    let mut copies = Vec::new();
    let mut around: Vec<Vec<u32>> = vec![Vec::new(); positions.len()];
    for (index, triangle) in triangles.iter().enumerate() {
        for corner in triangle {
            around[*corner as usize].push(index as u32);
        }
    }
    for vertex in 0..around.len() as u32 {
        let fan = &around[vertex as usize];
        if fan.len() < 2 {
            continue;
        }
        let mut union = Union::new(fan.len());
        for first in 0..fan.len() {
            for second in first + 1..fan.len() {
                let shared = triangles[fan[first] as usize]
                    .iter()
                    .filter(|corner| {
                        **corner != vertex && triangles[fan[second] as usize].contains(corner)
                    })
                    .count();
                if shared > 0 {
                    union.join(first as u32, second as u32);
                }
            }
        }
        let (labels, count) = union.labels();
        for part in 1..count as u32 {
            let copy = positions.len() as u32;
            copies.push((copy, [vertex, vertex]));
            positions.push(positions[vertex as usize]);
            pinned.push(pinned[vertex as usize]);
            for (slot, label) in labels.iter().enumerate() {
                if *label == part {
                    for corner in &mut triangles[fan[slot] as usize] {
                        if *corner == vertex {
                            *corner = copy;
                        }
                    }
                }
            }
        }
    }
    copies
}
fn part_pinches(positions: &mut [Vec3], triangles: &[[u32; 3]], pinches: &[(u32, [u32; 2])]) {
    for &(copy, [vertex, _]) in pinches {
        let at = positions[copy as usize];
        if at.distance(positions[vertex as usize]) > PINCH_APART {
            continue;
        }
        let (around, corners) = triangles
            .iter()
            .filter(|triangle| triangle.contains(&copy))
            .flatten()
            .filter(|corner| **corner != copy)
            .fold((Vec3::ZERO, 0.0f32), |(sum, count), corner| {
                (sum + positions[*corner as usize], count + 1.0)
            });
        if corners > 0.0 {
            positions[copy as usize] = at + (around / corners - at) * PINCH_SHIFT;
        }
    }
}

fn sides_of(triangles: &[[u32; 3]]) -> Sides {
    let mut sides: Sides = HashMap::new();
    for (index, triangle) in triangles.iter().enumerate() {
        for side in 0..3 {
            sides
                .entry(key(triangle[side], triangle[(side + 1) % 3]))
                .or_default()
                .push(index as u32);
        }
    }
    sides
}

fn loops(triangles: &[[u32; 3]]) -> Vec<Vec<u32>> {
    let sides = sides_of(triangles);
    let mut next: HashMap<u32, u32> = HashMap::new();
    let mut starts: Vec<u32> = Vec::new();
    for triangle in triangles {
        for side in 0..3 {
            let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
            if sides[&key(a, b)].len() == 1 {
                next.insert(a, b);
                starts.push(a);
            }
        }
    }
    starts.sort_unstable();
    let mut visited: HashMap<u32, bool> = HashMap::new();
    let mut found = Vec::new();
    for start in starts {
        if visited.contains_key(&start) {
            continue;
        }
        let mut walk = vec![start];
        visited.insert(start, true);
        let mut at = start;
        let closed = loop {
            let Some(&after) = next.get(&at) else {
                break false;
            };
            if after == start {
                break true;
            }
            if visited.insert(after, true).is_some() {
                break false;
            }
            walk.push(after);
            at = after;
        };
        if closed {
            found.push(walk);
        }
    }
    found
}

fn fill_holes(positions: &[Vec3], triangles: &mut Vec<[u32; 3]>, reach: f32) {
    for hole in loops(triangles) {
        let perimeter: f32 = (0..hole.len())
            .map(|at| {
                positions[hole[at] as usize]
                    .distance(positions[hole[(at + 1) % hole.len()] as usize])
            })
            .sum();
        if hole.len() > HOLE_SIDES || perimeter > reach {
            continue;
        }
        let sides = sides_of(triangles);
        let mut open = hole;
        while open.len() >= 3 {
            let count = open.len();
            let ear = (0..count)
                .filter(|at| {
                    let before = open[(at + count - 1) % count];
                    let after = open[(at + 1) % count];
                    count == 3 || !sides.contains_key(&key(before, after))
                })
                .map(|at| {
                    let corners = [
                        open[(at + 1) % count],
                        open[at],
                        open[(at + count - 1) % count],
                    ];
                    (
                        quality(corners.map(|corner| positions[corner as usize])),
                        at,
                        corners,
                    )
                })
                .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
            let Some((quality, at, corners)) = ear else {
                break;
            };
            if quality <= 0.0 {
                break;
            }
            triangles.push(corners);
            open.remove(at);
        }
    }
}

fn improve(positions: &[Vec3], triangles: &mut [[u32; 3]]) {
    for _ in 0..FLIP_SWEEPS {
        let sides = sides_of(triangles);
        let mut keys: Vec<(u32, u32)> = sides
            .iter()
            .filter(|(_, triangles)| triangles.len() == 2)
            .map(|(key, _)| *key)
            .collect();
        keys.sort_unstable();
        let mut touched = vec![false; triangles.len()];
        let mut made: Vec<(u32, u32)> = Vec::new();
        let mut flipped = 0;
        for (a, b) in keys {
            let pair = &sides[&(a, b)];
            let (first, second) = (pair[0] as usize, pair[1] as usize);
            if touched[first] || touched[second] {
                continue;
            }
            let Some(swapped) = flip(positions, triangles[first], triangles[second], [a, b]) else {
                continue;
            };
            let diagonal = key(swapped[0][0], swapped[0][2]);
            if sides.contains_key(&diagonal) || made.contains(&diagonal) {
                continue;
            }
            made.push(diagonal);
            triangles[first] = swapped[0];
            triangles[second] = swapped[1];
            touched[first] = true;
            touched[second] = true;
            flipped += 1;
        }
        if flipped == 0 {
            return;
        }
    }
}

fn flip(
    positions: &[Vec3],
    first: [u32; 3],
    second: [u32; 3],
    hinge: [u32; 2],
) -> Option<[[u32; 3]; 2]> {
    let rotate = |triangle: [u32; 3]| {
        let at = (0..3).find(|at| !hinge.contains(&triangle[*at]))?;
        Some([triangle[at], triangle[(at + 1) % 3], triangle[(at + 2) % 3]])
    };
    let [c, a, b] = rotate(first)?;
    let [d, b2, a2] = rotate(second)?;
    if a2 != a || b2 != b {
        return None;
    }
    let point = |corner: u32| positions[corner as usize];
    let angle = |at: u32, x: u32, y: u32| {
        (point(x) - point(at))
            .normalize_or_zero()
            .dot((point(y) - point(at)).normalize_or_zero())
            .clamp(-1.0, 1.0)
            .acos()
    };
    if angle(c, a, b) + angle(d, b, a) <= std::f32::consts::PI + FLIP_MARGIN {
        return None;
    }
    let swapped = [[c, a, d], [d, b, c]];
    let normal = |triangle: [u32; 3]| {
        let [p, q, r] = triangle.map(point);
        (q - p).cross(r - p).normalize_or_zero()
    };
    let before = (normal(first) + normal(second)).normalize_or_zero();
    let proper = swapped.iter().all(|triangle| {
        is_proper(triangle.map(point)) && normal(*triangle).dot(before) >= FLIP_TURN
    });
    let better = swapped
        .iter()
        .map(|triangle| quality(triangle.map(point)))
        .fold(f32::MAX, f32::min)
        > [first, second]
            .iter()
            .map(|triangle| quality(triangle.map(point)))
            .fold(f32::MAX, f32::min);
    (proper && better).then_some(swapped)
}

fn remove_small_components(triangles: &mut Vec<[u32; 3]>) {
    let highest = triangles
        .iter()
        .flatten()
        .copied()
        .max()
        .map_or(0, |top| top + 1);
    let mut union = Union::new(highest as usize);
    for [a, b, c] in triangles.iter() {
        union.join(*a, *b);
        union.join(*a, *c);
    }
    let (labels, count) = union.labels();
    let mut sizes = vec![0usize; count];
    for triangle in triangles.iter() {
        sizes[labels[triangle[0] as usize] as usize] += 1;
    }
    triangles
        .retain(|triangle| sizes[labels[triangle[0] as usize] as usize] >= MIN_COMPONENT_TRIANGLES);
}

fn compact(
    positions: Vec<Vec3>,
    pinned: Vec<bool>,
    triangles: Vec<[u32; 3]>,
    index: &[u32],
    blended: &[(u32, [u32; 2])],
) -> Envelope {
    let mut slot = vec![UNMAPPED; positions.len()];
    let mut envelope = Envelope {
        positions: Vec::new(),
        pinned: Vec::new(),
        triangles: Vec::new(),
        index: Vec::new(),
        blended: Vec::new(),
    };
    for triangle in &triangles {
        for corner in triangle {
            if slot[*corner as usize] == UNMAPPED {
                slot[*corner as usize] = envelope.positions.len() as u32;
                envelope.positions.push(positions[*corner as usize]);
                envelope.pinned.push(pinned[*corner as usize]);
            }
        }
    }
    envelope.triangles = triangles
        .iter()
        .map(|triangle| triangle.map(|corner| slot[corner as usize]))
        .collect();
    envelope.blended = blended
        .iter()
        .map(|(vertex, sources)| {
            (
                slot[*vertex as usize],
                sources.map(|source| slot[source as usize]),
            )
        })
        .filter(|(vertex, sources)| *vertex != UNMAPPED && !sources.contains(&UNMAPPED))
        .collect();
    envelope.index = index
        .iter()
        .map(|cluster| match *cluster {
            u32::MAX => UNMAPPED,
            cluster => slot[cluster as usize],
        })
        .collect();
    envelope
}

fn mean_edge(positions: &[Vec3], triangles: &[[u32; 3]]) -> f32 {
    let total: f32 = triangles
        .iter()
        .flat_map(|triangle| (0..3).map(move |side| (triangle[side], triangle[(side + 1) % 3])))
        .map(|(a, b)| positions[a as usize].distance(positions[b as usize]))
        .sum();
    total / (triangles.len() * 3).max(1) as f32
}

fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

pub(crate) fn quality(corners: [Vec3; 3]) -> f32 {
    let longest = corners[0]
        .distance_squared(corners[1])
        .max(corners[1].distance_squared(corners[2]))
        .max(corners[2].distance_squared(corners[0]));
    if longest > 1e-14 {
        area(corners) * 2.0 / longest / 0.866
    } else {
        0.0
    }
}
