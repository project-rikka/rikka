use glam::Vec3;

use crate::geometry::{Growth, followed};

const ATTEMPTS: usize = 100;
const TURN: f32 = 0.2;
const SLIVER: f32 = 0.08;
const CORNER: f32 = 0.85;

#[derive(Clone, Debug)]
pub(crate) struct Decimator {
    positions: Vec<Vec3>,
    pinned: Vec<bool>,
    alive: Vec<bool>,
    triangles: Vec<[u32; 3]>,
    live: Vec<bool>,
    incident: Vec<Vec<u32>>,
    redirect: Vec<u32>,
    growth: Growth,
    touched: Vec<bool>,
    last: f32,
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
        let alive = incident
            .iter()
            .map(|triangles| !triangles.is_empty())
            .collect();
        Decimator {
            redirect: (0..positions.len() as u32).collect(),
            live: vec![true; triangles.len()],
            touched: vec![true; positions.len()],
            positions,
            pinned,
            alive,
            triangles,
            incident,
            growth: Growth::default(),
            last: 0.0,
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.alive.iter().filter(|alive| **alive).count()
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

    fn neighbours(&self, vertex: u32) -> Vec<u32> {
        let mut neighbours: Vec<u32> = self.incident[vertex as usize]
            .iter()
            .flat_map(|triangle| self.triangles[*triangle as usize])
            .filter(|corner| *corner != vertex)
            .collect();
        neighbours.sort_unstable();
        neighbours.dedup();
        neighbours
    }

    fn rim(&self, vertex: u32) -> Vec<u32> {
        let mut sides: Vec<u32> = Vec::new();
        for triangle in &self.incident[vertex as usize] {
            let corners = self.triangles[*triangle as usize];
            sides.extend(corners.iter().copied().filter(|corner| *corner != vertex));
        }
        sides.sort_unstable();
        sides
            .chunk_by(|a, b| a == b)
            .filter(|run| run.len() == 1)
            .map(|run| run[0])
            .collect()
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

    fn pass(&mut self, radius: f32) -> usize {
        if radius > self.last {
            self.touched.fill(true);
            self.last = radius;
        }
        let mut candidates: Vec<(f32, u32, u32)> = Vec::new();
        for (triangle, live) in self.triangles.iter().zip(&self.live) {
            if !*live {
                continue;
            }
            for side in 0..3 {
                let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
                let (a, b) = (a.min(b), a.max(b));
                if !self.touched[a as usize] && !self.touched[b as usize] {
                    continue;
                }
                let length = self.positions[a as usize].distance(self.positions[b as usize]);
                if length <= radius {
                    candidates.push((length, a, b));
                }
            }
        }
        candidates.sort_by(|x, y| x.1.cmp(&y.1).then(x.2.cmp(&y.2)));
        candidates.dedup_by(|later, earlier| later.1 == earlier.1 && later.2 == earlier.2);
        let mut valence = vec![u32::MAX; self.positions.len()];
        let mut costed: Vec<(f32, u32, u32)> = Vec::with_capacity(candidates.len());
        for (length, a, b) in candidates {
            for end in [a, b] {
                if valence[end as usize] == u32::MAX {
                    valence[end as usize] = self.neighbours(end).len() as u32;
                }
            }
            let spread = (valence[a as usize] + valence[b as usize]) as f32 * 0.5;
            costed.push((length * (1.0 + spread), a, b));
        }
        costed.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        let mut used = vec![false; self.positions.len()];
        let mut survivors = Vec::new();
        for (_, a, b) in costed {
            if used[a as usize]
                || used[b as usize]
                || !self.alive[a as usize]
                || !self.alive[b as usize]
            {
                continue;
            }
            if let Some(target) = self.joinable(a, b) {
                self.join(a, b, target);
                used[a as usize] = true;
                used[b as usize] = true;
                survivors.push(a);
            }
        }
        self.touched.fill(false);
        for survivor in &survivors {
            self.touched[*survivor as usize] = true;
            for neighbour in self.neighbours(*survivor) {
                self.touched[neighbour as usize] = true;
            }
        }
        survivors.len()
    }

    fn joinable(&self, a: u32, b: u32) -> Option<Vec3> {
        let shared: Vec<u32> = self.incident[a as usize]
            .iter()
            .copied()
            .filter(|triangle| self.triangles[*triangle as usize].contains(&b))
            .collect();
        if shared.is_empty() || shared.len() > 2 {
            return None;
        }
        let wings: Vec<u32> = {
            let mut wings: Vec<u32> = shared
                .iter()
                .flat_map(|triangle| self.triangles[*triangle as usize])
                .filter(|corner| *corner != a && *corner != b)
                .collect();
            wings.sort_unstable();
            wings
        };
        let around_a = self.neighbours(a);
        let around_b = self.neighbours(b);
        let common: Vec<u32> = around_a
            .iter()
            .copied()
            .filter(|vertex| around_b.binary_search(vertex).is_ok())
            .collect();
        if common != wings {
            return None;
        }
        let (rim_a, rim_b) = (self.rim(a), self.rim(b));
        if !rim_a.is_empty() && !rim_b.is_empty() && shared.len() == 2 {
            return None;
        }
        let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
        let (rank_a, rank_b) = (self.rank(a, &rim_a), self.rank(b, &rim_b));
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
                let before = corners.map(|corner| self.positions[corner as usize]);
                let after = corners.map(|corner| {
                    if corner == vertex {
                        target
                    } else {
                        self.positions[corner as usize]
                    }
                });
                if !acceptable(before, after) {
                    return None;
                }
            }
        }
        Some(target)
    }

    fn join(&mut self, a: u32, b: u32, target: Vec3) {
        let shared: Vec<u32> = self.incident[a as usize]
            .iter()
            .copied()
            .filter(|triangle| self.triangles[*triangle as usize].contains(&b))
            .collect();
        for triangle in &shared {
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
        self.redirect[b as usize] = a;
        self.positions[a as usize] = target;
        self.pinned[a as usize] |= self.pinned[b as usize];
        if self.incident[a as usize].is_empty() {
            self.alive[a as usize] = false;
        }
    }
}

fn acceptable(before: [Vec3; 3], after: [Vec3; 3]) -> bool {
    let quality = |corners: [Vec3; 3]| {
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
    };
    let (old, new) = (quality(before), quality(after));
    if new <= 1e-6 || (new < SLIVER && new < old) {
        return false;
    }
    let normal_before = (before[1] - before[0]).cross(before[2] - before[0]);
    let normal_after = (after[1] - after[0]).cross(after[2] - after[0]);
    normal_before
        .normalize_or_zero()
        .dot(normal_after.normalize_or_zero())
        >= TURN
}
