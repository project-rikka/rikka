use crate::ids::{EdgeId, FaceId, LoopId, NONE, VertId};
use crate::mesh::EditMesh;
use crate::sets::IdSet;

impl EditMesh {
    #[must_use]
    pub fn vert_edge_count(&self, vert: VertId) -> usize {
        self.vert_edges(vert).count()
    }

    #[must_use]
    pub fn vert_edge_count_at_most(&self, vert: VertId, limit: usize) -> usize {
        self.vert_edges(vert).take(limit).count()
    }

    #[must_use]
    pub fn vert_face_count(&self, vert: VertId) -> usize {
        self.vert_loops(vert).count()
    }

    #[must_use]
    pub fn vert_face_count_at_most(&self, vert: VertId, limit: usize) -> usize {
        self.vert_loops(vert).take(limit).count()
    }

    #[must_use]
    pub fn edge_face_count(&self, edge: EdgeId) -> usize {
        self.edge_loops(edge).count()
    }

    #[must_use]
    pub fn edge_face_count_at_most(&self, edge: EdgeId, limit: usize) -> usize {
        self.edge_loops(edge).take(limit).count()
    }

    #[must_use]
    pub fn is_vert_loose(&self, vert: VertId) -> bool {
        self.verts.records[vert.index()].edge == NONE
    }

    #[must_use]
    pub fn is_vert_wire(&self, vert: VertId) -> bool {
        !self.is_vert_loose(vert) && self.vert_edges(vert).all(|edge| self.is_edge_wire(edge))
    }

    #[must_use]
    pub fn is_vert_wire_endpoint(&self, vert: VertId) -> bool {
        self.vert_edge_count_at_most(vert, 2) == 1 && self.is_vert_wire(vert)
    }

    #[must_use]
    pub fn is_vert_boundary(&self, vert: VertId) -> bool {
        self.vert_edges(vert)
            .any(|edge| self.is_edge_boundary(edge))
    }

    #[must_use]
    pub fn is_vert_edge_pair(&self, vert: VertId) -> bool {
        self.vert_edge_count_at_most(vert, 3) == 2
    }

    #[must_use]
    pub fn vert_edge_pair(&self, vert: VertId) -> Option<(EdgeId, EdgeId)> {
        let first = self.vert_first_edge(vert)?;
        let second = self.disk_next(first, vert);
        (second != first && self.disk_next(second, vert) == first).then_some((first, second))
    }

    #[must_use]
    pub fn is_edge_wire(&self, edge: EdgeId) -> bool {
        self.edges.records[edge.index()].first_loop == NONE
    }

    #[must_use]
    pub fn is_edge_boundary(&self, edge: EdgeId) -> bool {
        let lp = self.edges.records[edge.index()].first_loop;
        lp != NONE && self.loops.records[lp as usize].radial_next == lp
    }

    #[must_use]
    pub fn is_edge_manifold(&self, edge: EdgeId) -> bool {
        let lp = self.edges.records[edge.index()].first_loop;
        if lp == NONE {
            return false;
        }
        let next = self.loops.records[lp as usize].radial_next;
        next != lp && self.loops.records[next as usize].radial_next == lp
    }

    #[must_use]
    pub fn is_edge_non_manifold(&self, edge: EdgeId) -> bool {
        self.edge_face_count_at_most(edge, 3) != 2
    }

    #[must_use]
    pub fn is_edge_contiguous(&self, edge: EdgeId) -> bool {
        match self.edge_loop_pair(edge) {
            Some((a, b)) => self.loop_vert(a) != self.loop_vert(b),
            None => false,
        }
    }

    #[must_use]
    pub fn is_edge_convex(&self, edge: EdgeId) -> bool {
        let Some((a, b)) = self.edge_loop_pair(edge) else {
            return true;
        };
        let normal_a = self.face_normal(self.loop_face(a));
        let normal_b = self.face_normal(self.loop_face(b));
        if normal_a == normal_b {
            return true;
        }
        let cross = normal_a.cross(normal_b);
        let along = self.position(self.loop_next_vert(a)) - self.position(self.loop_vert(a));
        along.dot(cross) > 0.0
    }

    #[must_use]
    pub fn is_vert_manifold(&self, vert: VertId) -> bool {
        let Some(first_edge) = self.vert_first_edge(vert) else {
            return false;
        };
        let mut loop_count = 0;
        let mut boundaries = 0;
        let mut start = self.edges.records[first_edge.index()].first_loop;
        for edge in self.vert_edges(vert) {
            let lp = self.edges.records[edge.index()].first_loop;
            if lp == NONE {
                return false;
            }
            let radial = self.loops.records[lp as usize].radial_next;
            if lp != self.loops.records[radial as usize].radial_next {
                return false;
            }
            if self.loops.records[lp as usize].vert == vert.0 {
                loop_count += 1;
            }
            if radial != lp {
                if self.loops.records[radial as usize].vert == vert.0 {
                    loop_count += 1;
                }
            } else {
                start = lp;
                boundaries += 1;
                if boundaries == 3 {
                    return false;
                }
            }
        }
        let start = LoopId(start);
        let mut step_edge = self.loop_edge(start);
        let first = match self.loop_vert(start) == vert {
            true => start,
            false => self.loop_next(start),
        };
        let mut region = 0;
        let mut current = Some(first);
        while let Some(lp) = current {
            region += 1;
            current = self.vert_step_fan_loop(lp, &mut step_edge);
            if current == Some(first) {
                break;
            }
        }
        loop_count == region
    }

    #[must_use]
    pub fn is_vert_manifold_region(&self, vert: VertId) -> bool {
        let Some(first) = self.vert_loops(vert).next() else {
            return true;
        };
        let (count, total) = self.loop_region_counts(first);
        count == total
    }

    #[must_use]
    pub fn loop_region_counts(&self, lp: LoopId) -> (usize, usize) {
        let vert = self.loop_vert(lp);
        let mut visited_loops: IdSet<LoopId> = IdSet::new();
        let mut visited_edges: IdSet<EdgeId> = IdSet::new();
        let mut pending = vec![self.loop_edge(lp)];
        visited_edges.insert(self.loop_edge(lp));
        while let Some(edge) = pending.pop() {
            for radial in self.edge_loops(edge) {
                let (corner, other) = if self.loop_vert(radial) == vert {
                    (radial, self.loop_edge(self.loop_prev(radial)))
                } else if self.loop_next_vert(radial) == vert {
                    let next = self.loop_next(radial);
                    (next, self.loop_edge(next))
                } else {
                    continue;
                };
                visited_loops.insert(corner);
                if visited_edges.insert(other) {
                    pending.push(other);
                }
            }
        }
        (visited_loops.len(), self.vert_face_count(vert))
    }

    #[must_use]
    pub fn vert_step_fan_loop(&self, lp: LoopId, step_edge: &mut EdgeId) -> Option<LoopId> {
        let previous = *step_edge;
        let next = if self.loop_edge(lp) == previous {
            self.loop_edge(self.loop_prev(lp))
        } else if self.loop_edge(self.loop_prev(lp)) == previous {
            self.loop_edge(lp)
        } else {
            return None;
        };
        if !self.is_edge_manifold(next) {
            return None;
        }
        *step_edge = next;
        self.edge_other_loop(next, lp)
    }

    #[must_use]
    pub fn edge_other_loop(&self, edge: EdgeId, lp: LoopId) -> Option<LoopId> {
        let base = match self.loop_edge(lp) == edge {
            true => lp,
            false => self.loop_prev(lp),
        };
        let other = self.loop_radial_next(base);
        if other == base {
            return None;
        }
        let vert = self.loop_vert(lp);
        if self.loop_vert(other) == vert {
            Some(other)
        } else if self.loop_next_vert(other) == vert {
            Some(self.loop_next(other))
        } else {
            None
        }
    }

    #[must_use]
    pub fn face_other_vert_loop(
        &self,
        face: FaceId,
        previous: VertId,
        vert: VertId,
    ) -> Option<LoopId> {
        let lp = self.face_vert_share_loop(face, vert)?;
        if self.loop_vert(self.loop_prev(lp)) == previous {
            Some(self.loop_next(lp))
        } else if self.loop_next_vert(lp) == previous {
            Some(self.loop_prev(lp))
        } else {
            None
        }
    }

    #[must_use]
    pub fn loop_other_vert_loop(&self, lp: LoopId, vert: VertId) -> LoopId {
        let edge = self.loop_edge(lp);
        let previous = self.edge_other_vert(edge, vert);
        if self.loop_vert(lp) == vert {
            match Some(self.loop_vert(self.loop_prev(lp))) == previous {
                true => self.loop_next(lp),
                false => self.loop_prev(lp),
            }
        } else if self.loop_vert(self.loop_prev(lp)) == vert {
            self.loop_prev(self.loop_prev(lp))
        } else {
            self.loop_next(self.loop_next(lp))
        }
    }

    #[must_use]
    pub fn loop_other_edge_loop(&self, lp: LoopId, vert: VertId) -> LoopId {
        match self.loop_vert(lp) == vert {
            true => self.loop_prev(lp),
            false => self.loop_next(lp),
        }
    }

    #[must_use]
    pub fn face_other_edge_loop(&self, face: FaceId, edge: EdgeId, vert: VertId) -> Option<LoopId> {
        let lp = self.face_edge_share_loop(face, edge)?;
        Some(self.loop_other_edge_loop(lp, vert))
    }

    #[must_use]
    pub fn face_vert_share_loop(&self, face: FaceId, vert: VertId) -> Option<LoopId> {
        self.face_loops(face).find(|&lp| self.loop_vert(lp) == vert)
    }

    #[must_use]
    pub fn face_edge_share_loop(&self, face: FaceId, edge: EdgeId) -> Option<LoopId> {
        self.edge_loops(edge).find(|&lp| self.loop_face(lp) == face)
    }

    #[must_use]
    pub fn is_vert_in_face(&self, vert: VertId, face: FaceId) -> bool {
        self.face_vert_share_loop(face, vert).is_some()
    }

    #[must_use]
    pub fn is_edge_in_face(&self, edge: EdgeId, face: FaceId) -> bool {
        self.face_edge_share_loop(face, edge).is_some()
    }

    #[must_use]
    pub fn verts_in_face_count(&self, face: FaceId, verts: &[VertId]) -> usize {
        self.face_verts(face)
            .filter(|vert| verts.contains(vert))
            .count()
    }

    #[must_use]
    pub fn is_loop_adjacent(&self, a: LoopId, b: LoopId) -> bool {
        self.loop_face(a) == self.loop_face(b) && (self.loop_next(a) == b || self.loop_prev(a) == b)
    }

    #[must_use]
    pub fn edge_loop_pair(&self, edge: EdgeId) -> Option<(LoopId, LoopId)> {
        let first = self.edge_first_loop(edge)?;
        let second = self.loop_radial_next(first);
        (second != first && self.loop_radial_next(second) == first).then_some((first, second))
    }

    #[must_use]
    pub fn edge_face_pair(&self, edge: EdgeId) -> Option<(FaceId, FaceId)> {
        self.edge_loop_pair(edge)
            .map(|(a, b)| (self.loop_face(a), self.loop_face(b)))
    }

    #[must_use]
    pub fn edge_ordered_verts(&self, edge: EdgeId) -> [VertId; 2] {
        match self.edge_first_loop(edge) {
            Some(lp) => [self.loop_vert(lp), self.loop_next_vert(lp)],
            None => self.edge_verts(edge),
        }
    }

    #[must_use]
    pub fn edge_share_vert(&self, a: EdgeId, b: EdgeId) -> Option<VertId> {
        let [a0, a1] = self.edge_verts(a);
        let pair = self.edge_verts(b);
        if pair.contains(&a0) {
            Some(a0)
        } else if pair.contains(&a1) {
            Some(a1)
        } else {
            None
        }
    }

    #[must_use]
    pub fn edge_share_face(&self, a: EdgeId, b: EdgeId) -> Option<FaceId> {
        self.edge_faces(a)
            .find(|&face| self.is_edge_in_face(b, face))
    }

    #[must_use]
    pub fn edge_share_quad(&self, a: EdgeId, b: EdgeId) -> bool {
        self.edge_loops(a).any(|lp| {
            let face = self.loop_face(lp);
            self.face_len(face) == 4 && self.is_edge_in_face(b, face)
        })
    }

    #[must_use]
    pub fn face_share_edge_count(&self, a: FaceId, b: FaceId) -> usize {
        self.face_edges(a)
            .filter(|&edge| self.is_edge_in_face(edge, b))
            .count()
    }

    #[must_use]
    pub fn face_share_vert_count(&self, a: FaceId, b: FaceId) -> usize {
        self.face_verts(a)
            .filter(|&vert| self.is_vert_in_face(vert, b))
            .count()
    }

    #[must_use]
    pub fn face_share_edge(&self, a: FaceId, b: FaceId) -> Option<EdgeId> {
        self.face_edges(a)
            .find(|&edge| self.is_edge_in_face(edge, b))
    }

    #[must_use]
    pub fn face_share_face_count(&self, a: FaceId, b: FaceId) -> usize {
        let mut count = 0;
        for edge in self.face_edges(a) {
            for face in self.edge_faces(edge) {
                if face != a && face != b && self.face_share_edge(face, b).is_some() {
                    count += 1;
                }
            }
        }
        count
    }

    #[must_use]
    pub fn vert_pair_share_face(
        &self,
        a: VertId,
        b: VertId,
        allow_adjacent: bool,
    ) -> Option<(LoopId, LoopId)> {
        for lp_a in self.vert_loops(a) {
            let face = self.loop_face(lp_a);
            if let Some(lp_b) = self.face_vert_share_loop(face, b)
                && (allow_adjacent || !self.is_loop_adjacent(lp_a, lp_b))
            {
                return Some((lp_a, lp_b));
            }
        }
        None
    }

    #[must_use]
    pub fn vert_pair_share_face_by_len(
        &self,
        a: VertId,
        b: VertId,
        allow_adjacent: bool,
    ) -> Option<(LoopId, LoopId)> {
        let mut best: Option<(LoopId, LoopId)> = None;
        for lp_a in self.vert_loops(a) {
            let face = self.loop_face(lp_a);
            let Some(lp_b) = self.face_vert_share_loop(face, b) else {
                continue;
            };
            if !allow_adjacent && self.is_loop_adjacent(lp_a, lp_b) {
                continue;
            }
            if best.is_none_or(|(best_a, _)| {
                self.face_len(face) < self.face_len(self.loop_face(best_a))
            }) {
                best = Some((lp_a, lp_b));
            }
        }
        best
    }

    #[must_use]
    pub fn vert_pair_shares_face(&self, a: VertId, b: VertId) -> bool {
        self.vert_faces(a).any(|face| self.is_vert_in_face(b, face))
    }

    #[must_use]
    pub fn vert_other_disk_edge(&self, vert: VertId, edge: EdgeId) -> Option<EdgeId> {
        let first = self.edge_first_loop(edge)?;
        let mut lp = first;
        let mut total = 0;
        loop {
            lp = self.loop_other_vert_loop(lp, vert);
            if !self.edge_has_vert(self.loop_edge(lp), vert) {
                lp = self.loop_prev(lp);
            }
            if !self.is_edge_manifold(self.loop_edge(lp)) {
                return None;
            }
            lp = self.loop_radial_next(lp);
            total += 1;
            if lp == first {
                break;
            }
        }
        total /= 2;
        for _ in 0..total {
            lp = self.loop_other_vert_loop(lp, vert);
            if !self.edge_has_vert(self.loop_edge(lp), vert) {
                lp = self.loop_prev(lp);
            }
            lp = self.loop_radial_next(lp);
        }
        Some(self.loop_edge(lp))
    }

    #[must_use]
    pub fn vert_faces_of_region(&self, lp: LoopId) -> Vec<LoopId> {
        let vert = self.loop_vert(lp);
        let mut visited_loops: IdSet<LoopId> = IdSet::new();
        let mut visited_edges: IdSet<EdgeId> = IdSet::new();
        let mut order = Vec::new();
        let mut pending = vec![self.loop_edge(lp), self.loop_edge(self.loop_prev(lp))];
        visited_loops.insert(lp);
        order.push(lp);
        for &edge in &pending {
            visited_edges.insert(edge);
        }
        while let Some(edge) = pending.pop() {
            for radial in self.edge_loops(edge) {
                let corner = if self.loop_vert(radial) == vert {
                    radial
                } else if self.loop_next_vert(radial) == vert {
                    self.loop_next(radial)
                } else {
                    continue;
                };
                if visited_loops.insert(corner) {
                    order.push(corner);
                }
                for other in [
                    self.loop_edge(corner),
                    self.loop_edge(self.loop_prev(corner)),
                ] {
                    if visited_edges.insert(other) {
                        pending.push(other);
                    }
                }
            }
        }
        order
    }
}
