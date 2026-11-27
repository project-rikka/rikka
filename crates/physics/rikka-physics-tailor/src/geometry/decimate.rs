use glam::Vec3;
use rustc_hash::FxHashMap;

use crate::geometry::{Growth, followed};

const ATTEMPTS: usize = 100;
const TURN: f32 = 0.2;
const SLIVER: f32 = 0.08;
const CORNER: f32 = 0.85;
const UNKNOWN: u32 = u32::MAX;

#[derive(Clone, Debug)]
pub(crate) struct Decimator {
    positions: Vec<Vec3>,
    pinned: Vec<bool>,
    alive: Vec<bool>,
    living: usize,
    triangles: Vec<[u32; 3]>,
    live: Vec<bool>,
    lengths: Vec<[f32; 3]>,
    shapes: Vec<Shape>,
    incident: Vec<Vec<u32>>,
    redirect: Vec<u32>,
    growth: Growth,
    touched: Touched,
    last: f32,
    scratch: Scratch,
    stars: Stars,
}

#[derive(Clone, Debug)]
struct Touched {
    flags: Vec<bool>,
    listed: Vec<u32>,
    everything: bool,
}

impl Touched {
    fn everything(count: usize) -> Touched {
        Touched {
            flags: vec![true; count],
            listed: Vec::new(),
            everything: true,
        }
    }

    fn touch_all(&mut self) {
        self.flags.fill(true);
        self.listed.clear();
        self.everything = true;
    }

    fn clear(&mut self) {
        match self.everything {
            true => self.flags.fill(false),
            false => {
                for vertex in &self.listed {
                    self.flags[*vertex as usize] = false;
                }
            }
        }
        self.listed.clear();
        self.everything = false;
    }

    fn touch(&mut self, vertex: u32) {
        if !self.flags[vertex as usize] {
            self.flags[vertex as usize] = true;
            self.listed.push(vertex);
        }
    }

    fn get(&self, vertex: u32) -> bool {
        self.flags[vertex as usize]
    }
}

#[derive(Clone, Debug, Default)]
struct Stars {
    version: Vec<u32>,
    clock: u32,
    valence: Vec<(u32, u32)>,
    rejected: FxHashMap<u64, u32>,
}

impl Stars {
    fn new(count: usize) -> Stars {
        Stars {
            version: vec![0; count],
            clock: 0,
            valence: vec![(UNKNOWN, 0); count],
            rejected: FxHashMap::default(),
        }
    }

    fn unchanged(&self, vertex: u32, since: u32) -> bool {
        self.version[vertex as usize] <= since
    }

    fn valence(&self, vertex: u32) -> Option<u32> {
        let (valence, since) = self.valence[vertex as usize];
        (valence != UNKNOWN && self.unchanged(vertex, since)).then_some(valence)
    }

    fn rejected(&self, a: u32, b: u32) -> bool {
        self.rejected
            .get(&edge_key(a, b))
            .is_some_and(|since| self.unchanged(a, *since) && self.unchanged(b, *since))
    }

    fn reject(&mut self, a: u32, b: u32) {
        self.rejected.insert(edge_key(a, b), self.clock);
    }

    fn advance(&mut self) {
        self.clock += 1;
    }

    fn change(&mut self, vertex: u32) {
        self.version[vertex as usize] = self.clock;
    }
}

fn edge_key(a: u32, b: u32) -> u64 {
    (u64::from(a) << 32) | u64::from(b)
}

#[derive(Clone, Debug, Default)]
struct Ring {
    neighbours: Vec<u32>,
    rim: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
struct Seen {
    marks: Vec<u32>,
    stamp: u32,
}

#[derive(Clone, Debug, Default)]
struct Scratch {
    candidates: Vec<(f32, u32, u32)>,
    seen: Seen,
    used: Vec<bool>,
    counts: Vec<u32>,
    survivors: Vec<u32>,
    ring: Ring,
    other: Ring,
}

#[derive(Clone, Copy)]
struct Shared {
    triangles: [u32; 2],
    count: usize,
}

impl Shared {
    fn triangles(&self) -> &[u32] {
        &self.triangles[..self.count]
    }
}

#[derive(Clone, Copy)]
struct Collapse {
    target: Vec3,
    shared: Shared,
}

pub(crate) struct Reduced {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) pinned: Vec<bool>,
    pub(crate) triangles: Vec<[u32; 3]>,
    pub(crate) index: Vec<u32>,
}

impl Decimator {
    pub(crate) fn new(
        positions: Vec<Vec3>,
        pinned: Vec<bool>,
        triangles: Vec<[u32; 3]>,
    ) -> Decimator {
        let mut incident = vec![Vec::new(); positions.len()];
        for (index, triangle) in triangles.iter().enumerate() {
            for corner in triangle {
                incident[*corner as usize].push(index as u32);
            }
        }
        let alive: Vec<bool> = incident
            .iter()
            .map(|triangles| !triangles.is_empty())
            .collect();
        let count = positions.len();
        let mut decimator = Decimator {
            redirect: (0..count as u32).collect(),
            live: vec![true; triangles.len()],
            lengths: vec![[0.0; 3]; triangles.len()],
            shapes: vec![Shape::default(); triangles.len()],
            touched: Touched::everything(count),
            living: alive.iter().filter(|alive| **alive).count(),
            positions,
            pinned,
            alive,
            triangles,
            incident,
            growth: Growth::default(),
            last: 0.0,
            scratch: Scratch {
                used: vec![false; count],
                counts: vec![0; count],
                seen: Seen {
                    marks: vec![0; count],
                    stamp: 0,
                },
                ..Scratch::default()
            },
            stars: Stars::new(count),
        };
        for triangle in 0..decimator.triangles.len() {
            decimator.measure(triangle);
        }
        decimator
    }

    pub(crate) fn count(&self) -> usize {
        self.living
    }

    pub(crate) fn reduce(&mut self, start: f32, target: f32) {
        let mut growth = self.growth;
        growth.grow(start, target, |radius| self.pass(radius));
        self.growth = growth;
    }

    pub(crate) fn reduce_to(&mut self, start: f32, count: usize) {
        let mut radius = self.growth.reached().max(start);
        for _ in 0..ATTEMPTS {
            if self.count() <= count {
                break;
            }
            radius *= 1.15;
            self.reduce(start, radius);
        }
    }

    pub(crate) fn finish(&self) -> Reduced {
        let mut index = vec![u32::MAX; self.positions.len()];
        let mut reduced = Reduced {
            positions: Vec::new(),
            pinned: Vec::new(),
            triangles: Vec::new(),
            index: Vec::new(),
        };
        for (vertex, alive) in self.alive.iter().enumerate() {
            if *alive && !self.incident[vertex].is_empty() {
                index[vertex] = reduced.positions.len() as u32;
                reduced.positions.push(self.positions[vertex]);
                reduced.pinned.push(self.pinned[vertex]);
            }
        }
        for (triangle, live) in self.triangles.iter().zip(&self.live) {
            if *live {
                reduced
                    .triangles
                    .push(triangle.map(|corner| index[corner as usize]));
            }
        }
        reduced.index = (0..self.positions.len() as u32)
            .map(|vertex| index[followed(&self.redirect, vertex) as usize])
            .collect();
        reduced
    }

    fn measure(&mut self, triangle: usize) {
        let corners = self.triangles[triangle];
        self.lengths[triangle] = [0, 1, 2].map(|side| {
            let (a, b) = (corners[side], corners[(side + 1) % 3]);
            let (a, b) = (a.min(b), a.max(b));
            self.positions[a as usize].distance(self.positions[b as usize])
        });
        self.shapes[triangle] = Shape::of(corners.map(|corner| self.positions[corner as usize]));
    }

    fn ring(&self, vertex: u32, counts: &mut [u32], ring: &mut Ring) {
        ring.neighbours.clear();
        ring.rim.clear();
        for triangle in &self.incident[vertex as usize] {
            for corner in self.triangles[*triangle as usize] {
                if corner == vertex {
                    continue;
                }
                if counts[corner as usize] == 0 {
                    ring.neighbours.push(corner);
                }
                counts[corner as usize] += 1;
            }
        }
        for neighbour in &ring.neighbours {
            if counts[*neighbour as usize] == 1 {
                ring.rim.push(*neighbour);
            }
            counts[*neighbour as usize] = 0;
        }
        ring.rim.sort_unstable();
    }

    fn rank(&self, vertex: u32, rim: &[u32]) -> u8 {
        if self.pinned[vertex as usize] {
            return 3;
        }
        let at = self.positions[vertex as usize];
        match rim {
            [] => 0,
            [before, after] => {
                let incoming = at - self.positions[*before as usize];
                let outgoing = self.positions[*after as usize] - at;
                if incoming
                    .normalize_or_zero()
                    .dot(outgoing.normalize_or_zero())
                    < CORNER
                {
                    2
                } else {
                    1
                }
            }
            _ => 2,
        }
    }

    fn gather(&self, radius: f32, candidates: &mut Vec<(f32, u32, u32)>, seen: &mut Seen) {
        candidates.clear();
        match self.touched.everything {
            true => {
                let mut keyed: Vec<(u64, f32)> = Vec::new();
                for (triangle, live) in self.live.iter().enumerate() {
                    if !*live {
                        continue;
                    }
                    let corners = self.triangles[triangle];
                    for (side, length) in self.lengths[triangle].into_iter().enumerate() {
                        if length <= radius {
                            let (a, b) = (corners[side], corners[(side + 1) % 3]);
                            keyed.push((edge_key(a.min(b), a.max(b)), length));
                        }
                    }
                }
                keyed.sort_unstable_by_key(|(key, _)| *key);
                keyed.dedup_by_key(|(key, _)| *key);
                candidates.extend(
                    keyed
                        .into_iter()
                        .map(|(key, length)| (length, (key >> 32) as u32, key as u32)),
                );
            }
            false => {
                for vertex in &self.touched.listed {
                    let first = |other: u32| *vertex < other || !self.touched.get(other);
                    self.edges(*vertex, radius, first, seen, candidates);
                }
            }
        }
    }

    fn edges(
        &self,
        vertex: u32,
        radius: f32,
        owns: impl Fn(u32) -> bool,
        seen: &mut Seen,
        candidates: &mut Vec<(f32, u32, u32)>,
    ) {
        seen.stamp += 1;
        for triangle in &self.incident[vertex as usize] {
            let corners = self.triangles[*triangle as usize];
            let lengths = self.lengths[*triangle as usize];
            for side in 0..3 {
                let (a, b) = (corners[side], corners[(side + 1) % 3]);
                let other = match (a == vertex, b == vertex) {
                    (true, _) => b,
                    (_, true) => a,
                    _ => continue,
                };
                if lengths[side] <= radius
                    && owns(other)
                    && seen.marks[other as usize] != seen.stamp
                {
                    seen.marks[other as usize] = seen.stamp;
                    candidates.push((lengths[side], a.min(b), a.max(b)));
                }
            }
        }
    }

    fn pass(&mut self, radius: f32) -> usize {
        if radius > self.last {
            self.touched.touch_all();
            self.last = radius;
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut stars = std::mem::take(&mut self.stars);
        self.gather(radius, &mut scratch.candidates, &mut scratch.seen);
        for candidate in &mut scratch.candidates {
            let (length, a, b) = *candidate;
            let mut valence = |end: u32| match stars.valence(end) {
                Some(valence) => valence,
                None => {
                    self.ring(end, &mut scratch.counts, &mut scratch.ring);
                    let valence = scratch.ring.neighbours.len() as u32;
                    stars.valence[end as usize] = (valence, stars.clock);
                    valence
                }
            };
            let spread = (valence(a) + valence(b)) as f32 * 0.5;
            candidate.0 = length * (1.0 + spread);
        }
        scratch
            .candidates
            .sort_unstable_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        scratch.survivors.clear();
        for at in 0..scratch.candidates.len() {
            let (_, a, b) = scratch.candidates[at];
            if scratch.used[a as usize]
                || scratch.used[b as usize]
                || !self.alive[a as usize]
                || !self.alive[b as usize]
                || stars.rejected(a, b)
            {
                continue;
            }
            match self.joinable(a, b, &mut scratch) {
                Some(collapse) => {
                    self.join(a, b, collapse);
                    self.restar(a, collapse.shared, &mut stars, &mut scratch);
                    scratch.used[a as usize] = true;
                    scratch.used[b as usize] = true;
                    scratch.survivors.push(a);
                }
                None => stars.reject(a, b),
            }
        }
        self.touched.clear();
        for at in 0..scratch.survivors.len() {
            let survivor = scratch.survivors[at];
            self.touched.touch(survivor);
            self.ring(survivor, &mut scratch.counts, &mut scratch.ring);
            for neighbour in &scratch.ring.neighbours {
                self.touched.touch(*neighbour);
            }
        }
        for (_, a, b) in &scratch.candidates {
            scratch.used[*a as usize] = false;
            scratch.used[*b as usize] = false;
        }
        let joined = scratch.survivors.len();
        self.scratch = scratch;
        self.stars = stars;
        joined
    }

    fn shared(&self, a: u32, b: u32) -> Option<Shared> {
        let mut shared = Shared {
            triangles: [0; 2],
            count: 0,
        };
        for triangle in &self.incident[a as usize] {
            if self.triangles[*triangle as usize].contains(&b) {
                if shared.count == shared.triangles.len() {
                    return None;
                }
                shared.triangles[shared.count] = *triangle;
                shared.count += 1;
            }
        }
        (shared.count > 0).then_some(shared)
    }

    fn joinable(&self, a: u32, b: u32, scratch: &mut Scratch) -> Option<Collapse> {
        let shared = self.shared(a, b)?;
        let mut wings = [0u32; 6];
        let mut winged = 0;
        for triangle in shared.triangles() {
            for corner in self.triangles[*triangle as usize] {
                if corner != a && corner != b {
                    wings[winged] = corner;
                    winged += 1;
                }
            }
        }
        let wings = &mut wings[..winged];
        wings.sort_unstable();
        if wings.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        self.ring(a, &mut scratch.counts, &mut scratch.ring);
        self.ring(b, &mut scratch.counts, &mut scratch.other);
        for neighbour in &scratch.ring.neighbours {
            scratch.counts[*neighbour as usize] = 1;
        }
        let common = scratch
            .other
            .neighbours
            .iter()
            .filter(|neighbour| scratch.counts[**neighbour as usize] == 1)
            .count();
        for neighbour in &scratch.ring.neighbours {
            scratch.counts[*neighbour as usize] = 0;
        }
        if common != wings.len() {
            return None;
        }
        let (rim_a, rim_b) = (&scratch.ring.rim, &scratch.other.rim);
        if !rim_a.is_empty() && !rim_b.is_empty() && shared.count == 2 {
            return None;
        }
        let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
        let (rank_a, rank_b) = (self.rank(a, rim_a), self.rank(b, rim_b));
        let target = match rank_a.cmp(&rank_b) {
            std::cmp::Ordering::Greater => pa,
            std::cmp::Ordering::Less => pb,
            std::cmp::Ordering::Equal if rank_a == 2 => return None,
            std::cmp::Ordering::Equal => (pa + pb) * 0.5,
        };
        for (vertex, other) in [(a, b), (b, a)] {
            for triangle in &self.incident[vertex as usize] {
                let corners = self.triangles[*triangle as usize];
                if corners.contains(&other) {
                    continue;
                }
                let after = corners.map(|corner| {
                    if corner == vertex {
                        target
                    } else {
                        self.positions[corner as usize]
                    }
                });
                if !acceptable(self.shapes[*triangle as usize], after) {
                    return None;
                }
            }
        }
        Some(Collapse { target, shared })
    }

    fn join(&mut self, a: u32, b: u32, collapse: Collapse) {
        for triangle in collapse.shared.triangles() {
            self.live[*triangle as usize] = false;
            for corner in self.triangles[*triangle as usize] {
                self.incident[corner as usize].retain(|other| other != triangle);
            }
        }
        let moved = std::mem::take(&mut self.incident[b as usize]);
        for triangle in &moved {
            for corner in &mut self.triangles[*triangle as usize] {
                if *corner == b {
                    *corner = a;
                }
            }
        }
        self.incident[a as usize].extend(moved);
        self.incident[a as usize].sort_unstable();
        self.incident[a as usize].dedup();
        self.alive[b as usize] = false;
        self.living -= 1;
        self.redirect[b as usize] = a;
        self.positions[a as usize] = collapse.target;
        self.pinned[a as usize] |= self.pinned[b as usize];
        for at in 0..self.incident[a as usize].len() {
            self.measure(self.incident[a as usize][at] as usize);
        }
        if self.incident[a as usize].is_empty() {
            self.alive[a as usize] = false;
            self.living -= 1;
        }
    }

    fn restar(&self, survivor: u32, shared: Shared, stars: &mut Stars, scratch: &mut Scratch) {
        stars.advance();
        stars.change(survivor);
        for triangle in shared.triangles() {
            for corner in self.triangles[*triangle as usize] {
                stars.change(corner);
            }
        }
        self.ring(survivor, &mut scratch.counts, &mut scratch.ring);
        for neighbour in &scratch.ring.neighbours {
            stars.change(*neighbour);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Shape {
    quality: f32,
    normal: Vec3,
}

impl Shape {
    fn of(corners: [Vec3; 3]) -> Shape {
        Shape {
            quality: quality(corners),
            normal: (corners[1] - corners[0])
                .cross(corners[2] - corners[0])
                .normalize_or_zero(),
        }
    }
}

fn quality(corners: [Vec3; 3]) -> f32 {
    let longest = corners[0]
        .distance_squared(corners[1])
        .max(corners[1].distance_squared(corners[2]))
        .max(corners[2].distance_squared(corners[0]));
    let doubled = (corners[1] - corners[0])
        .cross(corners[2] - corners[0])
        .length();
    if longest > 1e-14 {
        doubled / longest
    } else {
        0.0
    }
}

fn acceptable(before: Shape, after: [Vec3; 3]) -> bool {
    let new = quality(after);
    if new <= 1e-6 || (new < SLIVER && new < before.quality) {
        return false;
    }
    let normal_after = (after[1] - after[0]).cross(after[2] - after[0]);
    before.normal.dot(normal_after.normalize_or_zero()) >= TURN
}
