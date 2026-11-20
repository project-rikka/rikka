use smallvec::SmallVec;

use crate::cdt::EdgeOrigin;
use crate::cdt::kernel::Kernel;
use crate::cdt::triangulation::{EdgeInfo, GHOST, Origin, Triangulation, edge_key};

struct Crossing {
    tri: u32,
    right: u32,
    left: u32,
}

enum Start {
    Exists,
    Collinear(u32),
    Cross(Crossing),
}

impl<K: Kernel> Triangulation<K> {
    pub(crate) fn add_constraint_edge(
        &mut self,
        a: u32,
        b: u32,
        support: &K::Support,
        kind: EdgeOrigin,
    ) {
        let origin = Origin {
            kind,
            forward: a < b,
        };
        let entry = self
            .constraints
            .entry(edge_key(a, b))
            .or_insert_with(|| EdgeInfo {
                origins: SmallVec::new(),
                support: support.clone(),
            });
        entry.origins.push(origin);
    }

    pub(crate) fn insert_segment(
        &mut self,
        from: u32,
        to: u32,
        support: K::Support,
        kind: EdgeOrigin,
    ) {
        if let Some(line) = self.line.clone() {
            self.insert_line_segment(&line, from, to, &support, kind);
            return;
        }
        let mut stack: Vec<(u32, u32)> = vec![(from, to)];
        while let Some((a, b)) = stack.pop() {
            if a == b || self.kernel.equal(a, b) {
                continue;
            }
            match self.segment_start(a, b) {
                Start::Exists => self.add_constraint_edge(a, b, &support, kind),
                Start::Collinear(w) => {
                    self.add_constraint_edge(a, w, &support, kind);
                    stack.push((w, b));
                }
                Start::Cross(crossing) => {
                    let next = self.cross_segment((a, b), crossing, &support, kind);
                    stack.extend(next);
                }
            }
        }
    }

    fn insert_line_segment(
        &mut self,
        line: &[u32],
        from: u32,
        to: u32,
        support: &K::Support,
        kind: EdgeOrigin,
    ) {
        let Some(start) = line.iter().position(|&v| v == from) else {
            return;
        };
        let Some(end) = line.iter().position(|&v| v == to) else {
            return;
        };
        let (low, high) = (start.min(end), start.max(end));
        for index in low..high {
            let (x, y) = match start < end {
                true => (line[index], line[index + 1]),
                false => (line[index + 1], line[index]),
            };
            self.add_constraint_edge(x, y, support, kind);
        }
    }

    fn segment_start(&self, a: u32, b: u32) -> Start {
        let fan = self.fan(a);
        for &tri in &fan {
            let record = self.tris[tri as usize];
            if record.v.contains(&b) {
                return Start::Exists;
            }
        }
        for &tri in &fan {
            let record = self.tris[tri as usize];
            if record.is_ghost() {
                continue;
            }
            let k = record.index_of(a).expect("fan triangle holds vertex");
            let x = record.v[(k + 1) % 3];
            let y = record.v[(k + 2) % 3];
            let ox = self.kernel.orient(a, b, x);
            if ox == 0 && self.kernel.between(a, b, x) {
                return Start::Collinear(x);
            }
            let oy = self.kernel.orient(a, b, y);
            if oy == 0 && self.kernel.between(a, b, y) {
                return Start::Collinear(y);
            }
            if ox < 0 && oy > 0 {
                return Start::Cross(Crossing {
                    tri,
                    right: x,
                    left: y,
                });
            }
        }
        unreachable!("segment leaves its start vertex through some triangle");
    }

    fn cross_segment(
        &mut self,
        segment: (u32, u32),
        crossing: Crossing,
        support: &K::Support,
        kind: EdgeOrigin,
    ) -> Vec<(u32, u32)> {
        let (a, b) = segment;
        let Crossing {
            tri: start,
            right,
            left,
        } = crossing;
        let mut region = vec![start];
        let mut crossed: Vec<(u32, u32)> = vec![(right, left)];
        let (mut r, mut l) = (right, left);
        let mut tri = start;
        let target = loop {
            let record = self.tris[tri as usize];
            let i = (0..3)
                .find(|&i| record.v[i] != r && record.v[i] != l)
                .expect("triangle has a third vertex");
            let next = record.n[i];
            let next_record = self.tris[next as usize];
            region.push(next);
            let z = next_record
                .v
                .iter()
                .copied()
                .find(|&v| v != r && v != l)
                .expect("neighbor has a third vertex");
            debug_assert_ne!(z, GHOST);
            if z == b {
                break b;
            }
            let orientation = self.kernel.orient(a, b, z);
            if orientation == 0 {
                break z;
            }
            match orientation < 0 {
                true => r = z,
                false => l = z,
            }
            crossed.push((r, l));
            tri = next;
        };
        if let Some(&(x, y)) = crossed.iter().find(|&&(x, y)| self.is_constrained(x, y)) {
            let edge_support = self.constraints[&edge_key(x, y)].support.clone();
            let point = self.kernel.intersection(support, &edge_support);
            self.insert_on_edge(x, y, point);
            return vec![(point, b), (a, point)];
        }
        let mut right_chain: Vec<u32> = Vec::new();
        let mut left_chain: Vec<u32> = Vec::new();
        for &(x, y) in &crossed {
            if right_chain.last() != Some(&x) {
                right_chain.push(x);
            }
            if left_chain.last() != Some(&y) {
                left_chain.push(y);
            }
        }
        let boundary = self.release_region(&region);
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        self.pseudo_polygon(a, target, &left_chain, &mut triangles);
        right_chain.reverse();
        self.pseudo_polygon(target, a, &right_chain, &mut triangles);
        self.create_region(&triangles, &boundary);
        self.add_constraint_edge(a, target, support, kind);
        match target == b {
            true => Vec::new(),
            false => vec![(target, b)],
        }
    }

    fn pseudo_polygon(&self, p: u32, q: u32, chain: &[u32], out: &mut Vec<[u32; 3]>) {
        if chain.is_empty() {
            return;
        }
        let mut best = 0;
        for index in 1..chain.len() {
            if self.kernel.in_circle(p, q, chain[best], chain[index]) > 0 {
                best = index;
            }
        }
        let c = chain[best];
        out.push([p, q, c]);
        self.pseudo_polygon(p, c, &chain[..best], out);
        self.pseudo_polygon(c, q, &chain[best + 1..], out);
    }
}
