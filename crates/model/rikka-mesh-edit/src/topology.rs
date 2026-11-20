use crate::error::{Error, Result};
use crate::ids::{EdgeId, FaceId, LoopId, NONE, VertId};
use crate::mesh::EditMesh;

impl EditMesh {
    #[must_use]
    pub fn vert_count(&self) -> usize {
        self.verts.live()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.live()
    }

    #[must_use]
    pub fn loop_count(&self) -> usize {
        self.loops.live()
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.faces.live()
    }

    #[must_use]
    pub fn vert_capacity(&self) -> usize {
        self.verts.capacity()
    }

    #[must_use]
    pub fn edge_capacity(&self) -> usize {
        self.edges.capacity()
    }

    #[must_use]
    pub fn loop_capacity(&self) -> usize {
        self.loops.capacity()
    }

    #[must_use]
    pub fn face_capacity(&self) -> usize {
        self.faces.capacity()
    }

    #[must_use]
    pub fn is_compact(&self) -> bool {
        self.verts.is_compact()
            && self.edges.is_compact()
            && self.loops.is_compact()
            && self.faces.is_compact()
    }

    #[must_use]
    pub fn contains_vert(&self, vert: VertId) -> bool {
        self.verts.is_alive(vert.0)
    }

    #[must_use]
    pub fn contains_edge(&self, edge: EdgeId) -> bool {
        self.edges.is_alive(edge.0)
    }

    #[must_use]
    pub fn contains_loop(&self, lp: LoopId) -> bool {
        self.loops.is_alive(lp.0)
    }

    #[must_use]
    pub fn contains_face(&self, face: FaceId) -> bool {
        self.faces.is_alive(face.0)
    }

    pub fn verts(&self) -> impl Iterator<Item = VertId> + '_ {
        self.verts.indices().map(VertId)
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.edges.indices().map(EdgeId)
    }

    pub fn loops(&self) -> impl Iterator<Item = LoopId> + '_ {
        self.loops.indices().map(LoopId)
    }

    pub fn faces(&self) -> impl Iterator<Item = FaceId> + '_ {
        self.faces.indices().map(FaceId)
    }

    #[must_use]
    pub fn vert_first_edge(&self, vert: VertId) -> Option<EdgeId> {
        debug_assert!(self.contains_vert(vert), "dead vertex {vert:?}");
        let edge = self.verts.records[vert.index()].edge;
        (edge != NONE).then_some(EdgeId(edge))
    }

    pub fn set_vert_first_edge(&mut self, vert: VertId, edge: EdgeId) -> Result<()> {
        self.check_vert(vert)?;
        self.check_edge(edge)?;
        if !self.edge_has_vert(edge, vert) {
            return Err(Error::VertNotInEdge(vert, edge));
        }
        self.verts.records[vert.index()].edge = edge.0;
        self.touch_topology();
        Ok(())
    }

    #[must_use]
    pub fn disk_next(&self, edge: EdgeId, vert: VertId) -> EdgeId {
        EdgeId(self.disk_next_raw(edge.0, vert.0))
    }

    #[must_use]
    pub fn disk_prev(&self, edge: EdgeId, vert: VertId) -> EdgeId {
        EdgeId(self.disk_prev_raw(edge.0, vert.0))
    }

    #[must_use]
    pub fn edge_verts(&self, edge: EdgeId) -> [VertId; 2] {
        debug_assert!(self.contains_edge(edge), "dead edge {edge:?}");
        self.edges.records[edge.index()].verts.map(VertId)
    }

    #[must_use]
    pub fn edge_other_vert(&self, edge: EdgeId, vert: VertId) -> Option<VertId> {
        let [a, b] = self.edge_verts(edge);
        match vert {
            _ if vert == a => Some(b),
            _ if vert == b => Some(a),
            _ => None,
        }
    }

    #[must_use]
    pub fn edge_has_vert(&self, edge: EdgeId, vert: VertId) -> bool {
        self.edge_verts(edge).contains(&vert)
    }

    #[must_use]
    pub fn edge_first_loop(&self, edge: EdgeId) -> Option<LoopId> {
        debug_assert!(self.contains_edge(edge), "dead edge {edge:?}");
        let lp = self.edges.records[edge.index()].first_loop;
        (lp != NONE).then_some(LoopId(lp))
    }

    #[must_use]
    pub fn loop_vert(&self, lp: LoopId) -> VertId {
        debug_assert!(self.contains_loop(lp), "dead loop {lp:?}");
        VertId(self.loops.records[lp.index()].vert)
    }

    #[must_use]
    pub fn loop_edge(&self, lp: LoopId) -> EdgeId {
        EdgeId(self.loops.records[lp.index()].edge)
    }

    #[must_use]
    pub fn loop_face(&self, lp: LoopId) -> FaceId {
        FaceId(self.loops.records[lp.index()].face)
    }

    #[must_use]
    pub fn loop_next(&self, lp: LoopId) -> LoopId {
        LoopId(self.loops.records[lp.index()].next)
    }

    #[must_use]
    pub fn loop_prev(&self, lp: LoopId) -> LoopId {
        LoopId(self.loops.records[lp.index()].prev)
    }

    #[must_use]
    pub fn loop_radial_next(&self, lp: LoopId) -> LoopId {
        LoopId(self.loops.records[lp.index()].radial_next)
    }

    #[must_use]
    pub fn loop_radial_prev(&self, lp: LoopId) -> LoopId {
        LoopId(self.loops.records[lp.index()].radial_prev)
    }

    #[must_use]
    pub fn loop_next_vert(&self, lp: LoopId) -> VertId {
        self.loop_vert(self.loop_next(lp))
    }

    #[must_use]
    pub fn face_first_loop(&self, face: FaceId) -> LoopId {
        debug_assert!(self.contains_face(face), "dead face {face:?}");
        LoopId(self.faces.records[face.index()].first)
    }

    #[must_use]
    pub fn face_len(&self, face: FaceId) -> usize {
        self.faces.records[face.index()].len as usize
    }

    pub fn vert_edges(&self, vert: VertId) -> impl Iterator<Item = EdgeId> + '_ {
        let first = self.verts.records[vert.index()].edge;
        let mut current = first;
        std::iter::from_fn(move || {
            if current == NONE {
                return None;
            }
            let edge = current;
            current = self.disk_next_raw(edge, vert.0);
            if current == first {
                current = NONE;
            }
            Some(EdgeId(edge))
        })
    }

    pub fn edge_loops(&self, edge: EdgeId) -> impl Iterator<Item = LoopId> + '_ {
        let first = self.edges.records[edge.index()].first_loop;
        self.cycle(first, |mesh, lp| {
            mesh.loops.records[lp as usize].radial_next
        })
    }

    pub fn edge_faces(&self, edge: EdgeId) -> impl Iterator<Item = FaceId> + '_ {
        self.edge_loops(edge).map(|lp| self.loop_face(lp))
    }

    pub fn loop_cycle(&self, lp: LoopId) -> impl Iterator<Item = LoopId> + '_ {
        self.cycle(lp.0, |mesh, lp| mesh.loops.records[lp as usize].next)
    }

    pub fn face_loops(&self, face: FaceId) -> impl Iterator<Item = LoopId> + '_ {
        let first = self.faces.records[face.index()].first;
        self.cycle(first, |mesh, lp| mesh.loops.records[lp as usize].next)
    }

    pub fn face_verts(&self, face: FaceId) -> impl Iterator<Item = VertId> + '_ {
        self.face_loops(face).map(|lp| self.loop_vert(lp))
    }

    pub fn face_edges(&self, face: FaceId) -> impl Iterator<Item = EdgeId> + '_ {
        self.face_loops(face).map(|lp| self.loop_edge(lp))
    }

    pub fn vert_loops(&self, vert: VertId) -> impl Iterator<Item = LoopId> + '_ {
        let first_edge = self.disk_face_edge_first(vert);
        let mut edge = first_edge;
        let mut start = first_edge.and_then(|edge| self.radial_face_loop_first(edge, vert));
        let mut current = start;
        std::iter::from_fn(move || {
            let lp = current?;
            let next = self.radial_face_loop_next(lp, vert);
            if Some(next) != start {
                current = Some(next);
            } else {
                let next_edge = edge.map(|edge| self.disk_face_edge_next(edge, vert));
                match next_edge == first_edge {
                    true => current = None,
                    false => {
                        edge = next_edge;
                        start = next_edge.and_then(|edge| self.radial_face_loop_first(edge, vert));
                        current = start;
                    }
                }
            }
            Some(lp)
        })
    }

    fn disk_face_edge_first(&self, vert: VertId) -> Option<EdgeId> {
        let first = self.vert_first_edge(vert)?;
        let mut edge = first;
        loop {
            if let Some(lp) = self.edge_first_loop(edge) {
                return Some(match self.loop_vert(lp) == vert {
                    true => edge,
                    false => self.loop_edge(self.loop_next(lp)),
                });
            }
            edge = self.disk_next(edge, vert);
            if edge == first {
                return None;
            }
        }
    }

    fn disk_face_edge_next(&self, edge: EdgeId, vert: VertId) -> EdgeId {
        let mut other = self.disk_next(edge, vert);
        while other != edge {
            if self
                .edge_first_loop(other)
                .is_some_and(|lp| self.radial_face_loop_first_from(lp, vert).is_some())
            {
                return other;
            }
            other = self.disk_next(other, vert);
        }
        edge
    }

    fn radial_face_loop_first(&self, edge: EdgeId, vert: VertId) -> Option<LoopId> {
        self.radial_face_loop_first_from(self.edge_first_loop(edge)?, vert)
    }

    fn radial_face_loop_first_from(&self, start: LoopId, vert: VertId) -> Option<LoopId> {
        let mut lp = start;
        loop {
            if self.loop_vert(lp) == vert {
                return Some(lp);
            }
            lp = self.loop_radial_next(lp);
            if lp == start {
                return None;
            }
        }
    }

    fn radial_face_loop_next(&self, lp: LoopId, vert: VertId) -> LoopId {
        let mut other = self.loop_radial_next(lp);
        while other != lp {
            if self.loop_vert(other) == vert {
                return other;
            }
            other = self.loop_radial_next(other);
        }
        lp
    }

    pub fn vert_faces(&self, vert: VertId) -> impl Iterator<Item = FaceId> + '_ {
        self.vert_loops(vert).map(|lp| self.loop_face(lp))
    }

    pub fn vert_verts(&self, vert: VertId) -> impl Iterator<Item = VertId> + '_ {
        self.vert_edges(vert).map(move |edge| {
            let [a, b] = self.edges.records[edge.index()].verts;
            VertId(if a == vert.0 { b } else { a })
        })
    }

    pub fn face_neighbours(&self, face: FaceId) -> impl Iterator<Item = (LoopId, LoopId)> + '_ {
        self.face_loops(face).flat_map(move |lp| {
            let mut radial = self.loop_radial_next(lp);
            std::iter::from_fn(move || {
                if radial == lp {
                    return None;
                }
                let other = radial;
                radial = self.loop_radial_next(radial);
                Some((lp, other))
            })
        })
    }

    pub fn face_verts_vec(&self, face: FaceId) -> Vec<VertId> {
        self.face_verts(face).collect()
    }

    pub fn face_loops_vec(&self, face: FaceId) -> Vec<LoopId> {
        self.face_loops(face).collect()
    }

    pub fn face_edges_vec(&self, face: FaceId) -> Vec<EdgeId> {
        self.face_edges(face).collect()
    }

    fn cycle(
        &self,
        first: u32,
        step: fn(&EditMesh, u32) -> u32,
    ) -> impl Iterator<Item = LoopId> + '_ {
        let mut current = first;
        std::iter::from_fn(move || {
            if current == NONE {
                return None;
            }
            let lp = current;
            current = step(self, lp);
            if current == first {
                current = NONE;
            }
            Some(LoopId(lp))
        })
    }
}
