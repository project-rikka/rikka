use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::cdt::EdgeOrigin;
use crate::cdt::kernel::Kernel;

pub(crate) const GHOST: u32 = u32::MAX;
pub(crate) const NONE: u32 = u32::MAX;
const DEAD: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tri {
    pub(crate) v: [u32; 3],
    pub(crate) n: [u32; 3],
}

impl Tri {
    pub(crate) fn is_ghost(&self) -> bool {
        self.v.contains(&GHOST)
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.v[0] != DEAD
    }

    pub(crate) fn index_of(&self, vertex: u32) -> Option<usize> {
        self.v.iter().position(|&v| v == vertex)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Origin {
    pub(crate) kind: EdgeOrigin,
    pub(crate) forward: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EdgeInfo<S> {
    pub(crate) origins: SmallVec<[Origin; 2]>,
    pub(crate) support: S,
}

pub(crate) fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

enum Location {
    Inside(u32),
    OnEdge(u32, usize),
    OnVertex(u32),
}

#[derive(Debug)]
pub(crate) struct Triangulation<K: Kernel> {
    pub(crate) kernel: K,
    pub(crate) tris: Vec<Tri>,
    free: Vec<u32>,
    vert_tri: Vec<u32>,
    pub(crate) constraints: FxHashMap<(u32, u32), EdgeInfo<K::Support>>,
    last: u32,
    seed: u64,
    pub(crate) line: Option<Vec<u32>>,
}

impl<K: Kernel> Triangulation<K> {
    pub(crate) fn new(kernel: K) -> Triangulation<K> {
        Triangulation {
            kernel,
            tris: Vec::new(),
            free: Vec::new(),
            vert_tri: Vec::new(),
            constraints: FxHashMap::default(),
            last: NONE,
            seed: 0x9e37_79b9_7f4a_7c15,
            line: None,
        }
    }

    fn random(&mut self) -> usize {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed % 3) as usize
    }

    fn ensure_vertex_slots(&mut self) {
        let count = self.kernel.vertex_count();
        if self.vert_tri.len() < count {
            self.vert_tri.resize(count, NONE);
        }
    }

    fn alloc(&mut self, v: [u32; 3]) -> u32 {
        let tri = Tri { v, n: [NONE; 3] };
        match self.free.pop() {
            Some(index) => {
                self.tris[index as usize] = tri;
                index
            }
            None => {
                self.tris.push(tri);
                (self.tris.len() - 1) as u32
            }
        }
    }

    fn release(&mut self, index: u32) {
        self.tris[index as usize].v = [DEAD; 3];
        self.free.push(index);
    }

    fn touch(&mut self, index: u32) {
        let tri = self.tris[index as usize];
        for vertex in tri.v {
            if vertex != GHOST {
                self.vert_tri[vertex as usize] = index;
            }
        }
        if !tri.is_ghost() {
            self.last = index;
        }
    }

    fn set_tri(&mut self, index: u32, v: [u32; 3], n: [u32; 3]) {
        self.tris[index as usize] = Tri { v, n };
    }

    fn replace_neighbor(&mut self, tri: u32, old: u32, new: u32) {
        if tri == NONE {
            return;
        }
        let record = &mut self.tris[tri as usize];
        if let Some(slot) = record.n.iter_mut().find(|slot| **slot == old) {
            *slot = new;
        }
    }

    pub(crate) fn is_constrained(&self, a: u32, b: u32) -> bool {
        a != GHOST && b != GHOST && self.constraints.contains_key(&edge_key(a, b))
    }

    pub(crate) fn build(&mut self, order: &[u32]) -> Vec<u32> {
        self.ensure_vertex_slots();
        let mut representative: Vec<u32> = (0..self.kernel.vertex_count() as u32).collect();
        let Some(&first) = order.first() else {
            return representative;
        };
        let second = order.iter().copied().find(|&v| !self.same(first, v));
        let Some(second) = second else {
            for &v in order {
                representative[v as usize] = first;
            }
            self.line = Some(vec![first]);
            return representative;
        };
        let third = order
            .iter()
            .copied()
            .find(|&v| self.kernel.orient(first, second, v) != 0);
        let Some(third) = third else {
            self.line = Some(self.collinear_line(order, &mut representative));
            return representative;
        };
        let (b, c) = match self.kernel.orient(first, second, third) > 0 {
            true => (second, third),
            false => (third, second),
        };
        self.init_triangle(first, b, c);
        for &v in order {
            if v == first || v == second || v == third {
                continue;
            }
            representative[v as usize] = self.insert(v);
        }
        representative
    }

    fn same(&self, a: u32, b: u32) -> bool {
        self.kernel.equal(a, b)
    }

    fn collinear_line(&self, order: &[u32], representative: &mut [u32]) -> Vec<u32> {
        let mut line: Vec<u32> = order.to_vec();
        line.sort_by(|&p, &q| self.kernel.compare(p, q).then(p.cmp(&q)));
        let mut unique: Vec<u32> = Vec::with_capacity(line.len());
        for v in line {
            match unique.last() {
                Some(&last) if self.same(last, v) => representative[v as usize] = last,
                _ => unique.push(v),
            }
        }
        unique
    }

    fn init_triangle(&mut self, a: u32, b: u32, c: u32) {
        let real = self.alloc([a, b, c]);
        let g0 = self.alloc([c, b, GHOST]);
        let g1 = self.alloc([a, c, GHOST]);
        let g2 = self.alloc([b, a, GHOST]);
        self.link(&[real, g0, g1, g2], &FxHashMap::default());
        for tri in [g0, g1, g2, real] {
            self.touch(tri);
        }
    }

    pub(crate) fn link(&mut self, created: &[u32], boundary: &FxHashMap<(u32, u32), u32>) {
        let mut half: FxHashMap<(u32, u32), (u32, usize)> = FxHashMap::default();
        for &tri in created {
            let v = self.tris[tri as usize].v;
            for i in 0..3 {
                half.insert((v[(i + 1) % 3], v[(i + 2) % 3]), (tri, i));
            }
        }
        for &tri in created {
            let v = self.tris[tri as usize].v;
            for i in 0..3 {
                let (x, y) = (v[(i + 1) % 3], v[(i + 2) % 3]);
                if let Some(&(other, j)) = half.get(&(y, x)) {
                    self.tris[tri as usize].n[i] = other;
                    self.tris[other as usize].n[j] = tri;
                } else if let Some(&outside) = boundary.get(&(x, y)) {
                    self.tris[tri as usize].n[i] = outside;
                    let record = self.tris[outside as usize];
                    for k in 0..3 {
                        if record.v[(k + 1) % 3] == y && record.v[(k + 2) % 3] == x {
                            self.tris[outside as usize].n[k] = tri;
                        }
                    }
                }
            }
        }
    }

    fn in_circle_tri(&self, tri: u32, p: u32) -> bool {
        let v = self.tris[tri as usize].v;
        match v.iter().position(|&x| x == GHOST) {
            Some(g) => {
                let (x, y) = (v[(g + 1) % 3], v[(g + 2) % 3]);
                let orientation = self.kernel.orient(x, y, p);
                orientation > 0 || (orientation == 0 && self.kernel.between(x, y, p))
            }
            None => self.kernel.in_circle(v[0], v[1], v[2], p) > 0,
        }
    }

    fn locate(&mut self, p: u32) -> (Location, u32) {
        let mut tri = self.last;
        let limit = self.tris.len() * 3 + 64;
        let mut steps = 0;
        loop {
            let record = self.tris[tri as usize];
            if record.is_ghost() {
                return (Location::Inside(tri), tri);
            }
            let start = self.random();
            let mut zeros: SmallVec<[usize; 3]> = SmallVec::new();
            let mut next = NONE;
            for k in 0..3 {
                let i = (start + k) % 3;
                let (a, b) = (record.v[(i + 1) % 3], record.v[(i + 2) % 3]);
                let orientation = self.kernel.orient(a, b, p);
                if orientation < 0 {
                    next = record.n[i];
                    break;
                }
                if orientation == 0 {
                    zeros.push(i);
                }
            }
            if next == NONE {
                return match zeros.len() {
                    0 => (Location::Inside(tri), tri),
                    1 => (Location::OnEdge(tri, zeros[0]), tri),
                    _ => {
                        let k = (0..3).find(|k| !zeros.contains(k)).unwrap_or(0);
                        (Location::OnVertex(record.v[k]), tri)
                    }
                };
            }
            tri = next;
            steps += 1;
            if steps > limit {
                return self.locate_scan(p);
            }
        }
    }

    fn locate_scan(&self, p: u32) -> (Location, u32) {
        for (index, record) in self.tris.iter().enumerate() {
            if !record.is_alive() || record.is_ghost() {
                continue;
            }
            let orientations: [i32; 3] = std::array::from_fn(|i| {
                self.kernel
                    .orient(record.v[(i + 1) % 3], record.v[(i + 2) % 3], p)
            });
            if orientations.iter().all(|&o| o >= 0) {
                let zeros: SmallVec<[usize; 3]> =
                    (0..3).filter(|&i| orientations[i] == 0).collect();
                let tri = index as u32;
                return match zeros.len() {
                    0 => (Location::Inside(tri), tri),
                    1 => (Location::OnEdge(tri, zeros[0]), tri),
                    _ => {
                        let k = (0..3).find(|k| !zeros.contains(k)).unwrap_or(0);
                        (Location::OnVertex(record.v[k]), tri)
                    }
                };
            }
        }
        for (index, record) in self.tris.iter().enumerate() {
            if record.is_alive() && record.is_ghost() && self.in_circle_tri(index as u32, p) {
                return (Location::Inside(index as u32), index as u32);
            }
        }
        (Location::Inside(self.last), self.last)
    }

    pub(crate) fn insert(&mut self, p: u32) -> u32 {
        self.ensure_vertex_slots();
        let (location, _) = self.locate(p);
        match location {
            Location::OnVertex(existing) => existing,
            Location::Inside(tri) => {
                self.split_triangle(tri, p);
                p
            }
            Location::OnEdge(tri, i) => {
                self.split_edge(tri, i, p);
                p
            }
        }
    }

    pub(crate) fn insert_on_edge(&mut self, a: u32, b: u32, p: u32) {
        self.ensure_vertex_slots();
        let candidates = [self.find_edge(a, b), self.find_edge(b, a)];
        let (tri, i) = candidates
            .iter()
            .flatten()
            .copied()
            .find(|&(tri, _)| !self.tris[tri as usize].is_ghost())
            .expect("constrained edge has a real triangle");
        self.split_edge(tri, i, p);
    }

    fn split_triangle(&mut self, tri: u32, p: u32) {
        let Tri { v: [a, b, c], n } = self.tris[tri as usize];
        let t1 = self.alloc([b, c, p]);
        let t2 = self.alloc([c, a, p]);
        self.set_tri(tri, [a, b, p], [t1, t2, n[2]]);
        self.set_tri(t1, [b, c, p], [t2, tri, n[0]]);
        self.set_tri(t2, [c, a, p], [tri, t1, n[1]]);
        self.replace_neighbor(n[0], tri, t1);
        self.replace_neighbor(n[1], tri, t2);
        for t in [tri, t1, t2] {
            self.touch(t);
        }
        self.legalize(vec![tri, t1, t2], p);
    }

    fn split_edge(&mut self, tri: u32, i: usize, p: u32) {
        let record = self.tris[tri as usize];
        let c = record.v[i];
        let a = record.v[(i + 1) % 3];
        let b = record.v[(i + 2) % 3];
        let other = record.n[i];
        let other_record = self.tris[other as usize];
        let j = (0..3)
            .find(|&j| other_record.v[j] != a && other_record.v[j] != b)
            .expect("neighbor shares edge");
        let d = other_record.v[j];
        let n_ca = record.n[(i + 2) % 3];
        let n_bc = record.n[(i + 1) % 3];
        let ia = other_record.index_of(a).expect("neighbor has a");
        let ib = other_record.index_of(b).expect("neighbor has b");
        let n_db = other_record.n[ia];
        let n_ad = other_record.n[ib];
        let t2 = self.alloc([c, p, b]);
        let o2 = self.alloc([d, p, a]);
        self.set_tri(tri, [c, a, p], [o2, t2, n_ca]);
        self.set_tri(t2, [c, p, b], [other, n_bc, tri]);
        self.set_tri(other, [d, b, p], [t2, o2, n_db]);
        self.set_tri(o2, [d, p, a], [tri, n_ad, other]);
        self.replace_neighbor(n_bc, tri, t2);
        self.replace_neighbor(n_ad, other, o2);
        if let Some(info) = self.constraints.remove(&edge_key(a, b)) {
            let forward_first = |x: u32, y: u32, origin: &Origin| -> Origin {
                let along = origin.forward == (a < b);
                let from_a_to_b = along;
                let direction = match from_a_to_b {
                    true => x < y,
                    false => y < x,
                };
                Origin {
                    kind: origin.kind,
                    forward: direction,
                }
            };
            let first = EdgeInfo {
                origins: info
                    .origins
                    .iter()
                    .map(|o| forward_first(a, p, o))
                    .collect(),
                support: info.support.clone(),
            };
            let second = EdgeInfo {
                origins: info
                    .origins
                    .iter()
                    .map(|o| forward_first(p, b, o))
                    .collect(),
                support: info.support,
            };
            self.constraints.insert(edge_key(a, p), first);
            self.constraints.insert(edge_key(p, b), second);
        }
        for t in [tri, t2, other, o2] {
            self.touch(t);
        }
        self.legalize(vec![tri, t2, other, o2], p);
    }

    fn legalize(&mut self, mut stack: Vec<u32>, p: u32) {
        while let Some(tri) = stack.pop() {
            let record = self.tris[tri as usize];
            let Some(i) = record.index_of(p) else {
                continue;
            };
            let a = record.v[(i + 1) % 3];
            let b = record.v[(i + 2) % 3];
            if self.is_constrained(a, b) {
                continue;
            }
            let other = record.n[i];
            if other == NONE || !self.in_circle_tri(other, p) {
                continue;
            }
            let other_record = self.tris[other as usize];
            let Some(j) = (0..3).find(|&j| other_record.v[j] != a && other_record.v[j] != b) else {
                continue;
            };
            let q = other_record.v[j];
            if q == GHOST && !record.is_ghost() {
                continue;
            }
            let ia = other_record.index_of(a).expect("neighbor has a");
            let ib = other_record.index_of(b).expect("neighbor has b");
            let n_aq = other_record.n[ib];
            let n_qb = other_record.n[ia];
            let n_bp = record.n[(i + 1) % 3];
            let n_pa = record.n[(i + 2) % 3];
            self.set_tri(tri, [p, a, q], [n_aq, other, n_pa]);
            self.set_tri(other, [p, q, b], [n_qb, n_bp, tri]);
            self.replace_neighbor(n_aq, other, tri);
            self.replace_neighbor(n_bp, tri, other);
            self.touch(tri);
            self.touch(other);
            stack.push(tri);
            stack.push(other);
        }
    }

    pub(crate) fn fan(&self, vertex: u32) -> Vec<u32> {
        let start = self.vert_tri[vertex as usize];
        let mut result = Vec::new();
        if start == NONE {
            return result;
        }
        let mut tri = start;
        loop {
            result.push(tri);
            let record = self.tris[tri as usize];
            let Some(i) = record.index_of(vertex) else {
                break;
            };
            tri = record.n[(i + 1) % 3];
            if tri == start || tri == NONE || result.len() > self.tris.len() {
                break;
            }
        }
        result
    }

    pub(crate) fn find_edge(&self, a: u32, b: u32) -> Option<(u32, usize)> {
        for tri in self.fan(a) {
            let record = self.tris[tri as usize];
            for i in 0..3 {
                if record.v[(i + 1) % 3] == a && record.v[(i + 2) % 3] == b {
                    return Some((tri, i));
                }
            }
        }
        None
    }

    pub(crate) fn release_region(&mut self, region: &[u32]) -> FxHashMap<(u32, u32), u32> {
        let members: rustc_hash::FxHashSet<u32> = region.iter().copied().collect();
        let mut boundary = FxHashMap::default();
        for &tri in region {
            let record = self.tris[tri as usize];
            for i in 0..3 {
                let neighbor = record.n[i];
                if !members.contains(&neighbor) {
                    boundary.insert((record.v[(i + 1) % 3], record.v[(i + 2) % 3]), neighbor);
                }
            }
        }
        for &tri in region {
            self.release(tri);
        }
        boundary
    }

    pub(crate) fn create_region(
        &mut self,
        triangles: &[[u32; 3]],
        boundary: &FxHashMap<(u32, u32), u32>,
    ) {
        let created: Vec<u32> = triangles.iter().map(|&v| self.alloc(v)).collect();
        self.link(&created, boundary);
        for tri in created {
            self.touch(tri);
        }
    }
}
