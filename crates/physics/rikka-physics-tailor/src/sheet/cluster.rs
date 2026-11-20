use glam::Vec3;

use crate::geometry::{Growth, PointGrid, followed};

const PINNED_SHARE: f32 = 0.5;

#[derive(Clone, Debug)]
pub(crate) struct Clusters {
    positions: Vec<Vec3>,
    pinned: Vec<bool>,
    alive: Vec<bool>,
    redirect: Vec<u32>,
    links: Vec<Vec<u32>>,
    growth: Growth,
}

pub(crate) struct Clustered {
    pub(crate) positions: Vec<Vec3>,
    pub(crate) pinned: Vec<bool>,
    pub(crate) index: Vec<u32>,
}

impl Clusters {
    pub(crate) fn new(positions: Vec<Vec3>, pinned: Vec<bool>, triangles: &[[u32; 3]]) -> Clusters {
        let mut links = vec![Vec::new(); positions.len()];
        for triangle in triangles {
            for side in 0..3 {
                let (a, b) = (triangle[side], triangle[(side + 1) % 3]);
                links[a as usize].push(b);
                links[b as usize].push(a);
            }
        }
        for neighbours in &mut links {
            neighbours.sort_unstable();
            neighbours.dedup();
        }
        Clusters {
            redirect: (0..positions.len() as u32).collect(),
            alive: vec![true; positions.len()],
            positions,
            pinned,
            links,
            growth: Growth::default(),
        }
    }

    pub(crate) fn reduce(&mut self, start: f32, target: f32) {
        let mut growth = self.growth;
        growth.grow(start, target, |radius| {
            self.pass(radius, target * PINNED_SHARE)
        });
        self.growth = growth;
    }

    pub(crate) fn finish(&self) -> Clustered {
        let mut compact = vec![u32::MAX; self.positions.len()];
        let mut clustered = Clustered {
            positions: Vec::new(),
            pinned: Vec::new(),
            index: Vec::new(),
        };
        for (node, alive) in self.alive.iter().enumerate() {
            if *alive {
                compact[node] = clustered.positions.len() as u32;
                clustered.positions.push(self.positions[node]);
                clustered.pinned.push(self.pinned[node]);
            }
        }
        clustered.index = (0..self.positions.len() as u32)
            .map(|node| compact[followed(&self.redirect, node) as usize])
            .collect();
        clustered
    }

    fn pass(&mut self, radius: f32, pinned_radius: f32) -> usize {
        let grid = PointGrid::new(
            (0..self.positions.len() as u32)
                .filter(|node| self.alive[*node as usize])
                .map(|node| (node, self.positions[node as usize])),
            radius,
        );
        let mut costed: Vec<(f32, u32, u32)> = Vec::new();
        for a in (0..self.positions.len() as u32).filter(|node| self.alive[*node as usize]) {
            let at = self.positions[a as usize];
            for b in grid.near(at, radius) {
                if b <= a {
                    continue;
                }
                let distance = at.distance(self.positions[b as usize]);
                let both_pinned = self.pinned[a as usize] && self.pinned[b as usize];
                if distance > radius || (both_pinned && distance > pinned_radius) {
                    continue;
                }
                let valence = (self.links[a as usize].len() + self.links[b as usize].len()) as f32;
                costed.push((distance * (1.0 + valence * 0.5), a, b));
            }
        }
        costed.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
        let mut used = vec![false; self.positions.len()];
        let mut joined = 0;
        for (_, a, b) in costed {
            if used[a as usize] || used[b as usize] || !self.joinable(a, b) {
                continue;
            }
            self.join(a, b);
            used[a as usize] = true;
            used[b as usize] = true;
            joined += 1;
        }
        joined
    }

    fn joinable(&self, a: u32, b: u32) -> bool {
        let (around_a, around_b) = (&self.links[a as usize], &self.links[b as usize]);
        let mut ring: Vec<u32> = around_a
            .iter()
            .chain(around_b)
            .copied()
            .filter(|node| *node != a && *node != b)
            .collect();
        ring.sort_unstable();
        ring.dedup();
        if ring.is_empty() {
            return false;
        }
        if around_a.binary_search(&b).is_err() {
            return true;
        }
        let mut reached = vec![false; ring.len()];
        let mut stack = vec![0usize];
        reached[0] = true;
        while let Some(at) = stack.pop() {
            for next in &self.links[ring[at] as usize] {
                if let Ok(slot) = ring.binary_search(next)
                    && !reached[slot]
                {
                    reached[slot] = true;
                    stack.push(slot);
                }
            }
        }
        reached.iter().all(|reached| *reached)
    }

    fn join(&mut self, a: u32, b: u32) {
        let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
        self.positions[a as usize] = match (self.pinned[a as usize], self.pinned[b as usize]) {
            (true, false) => pa,
            (false, true) => pb,
            _ => (pa + pb) * 0.5,
        };
        self.pinned[a as usize] |= self.pinned[b as usize];
        let moved = std::mem::take(&mut self.links[b as usize]);
        for neighbour in &moved {
            let neighbours = &mut self.links[*neighbour as usize];
            neighbours.retain(|node| *node != b);
            if *neighbour != a && neighbours.binary_search(&a).is_err() {
                let at = neighbours.partition_point(|node| *node < a);
                neighbours.insert(at, a);
            }
        }
        let merged = &mut self.links[a as usize];
        merged.extend(moved.into_iter().filter(|node| *node != a));
        merged.retain(|node| *node != b);
        merged.sort_unstable();
        merged.dedup();
        self.alive[b as usize] = false;
        self.redirect[b as usize] = a;
    }
}
