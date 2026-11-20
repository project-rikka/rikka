use crate::core::Ring;
use crate::error::{Error, Result};
use crate::ids::{EdgeId, FaceId, LoopId, NONE, VertId};
use crate::mesh::EditMesh;
use crate::sets::IdSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JoinOptions {
    pub check_edge_exists: bool,
    pub kill_duplicate_faces: bool,
}

impl Default for JoinOptions {
    fn default() -> JoinOptions {
        JoinOptions {
            check_edge_exists: true,
            kill_duplicate_faces: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DoubleFace {
    #[default]
    Reuse,
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoinedFace {
    pub face: FaceId,
    pub double: Option<FaceId>,
}

impl EditMesh {
    pub fn join_edge_kill_vert(
        &mut self,
        kill_edge: EdgeId,
        kill_vert: VertId,
        options: JoinOptions,
    ) -> Result<EdgeId> {
        self.check_edge(kill_edge)?;
        self.check_vert(kill_vert)?;
        if !self.edge_has_vert(kill_edge, kill_vert) {
            return Err(Error::VertNotInEdge(kill_vert, kill_edge));
        }
        if self.vert_edge_count_at_most(kill_vert, 3) != 2 {
            return Err(Error::NotTwoEdges(kill_vert));
        }
        let old_edge = self.disk_next(kill_edge, kill_vert);
        let target = self
            .edge_other_vert(kill_edge, kill_vert)
            .ok_or(Error::VertNotInEdge(kill_vert, kill_edge))?;
        let old_vert = self
            .edge_other_vert(old_edge, kill_vert)
            .ok_or(Error::VertNotInEdge(kill_vert, old_edge))?;
        if old_vert == target {
            return Err(Error::DoubleEdge);
        }
        let splice = match options.check_edge_exists {
            true => self.edge_between(target, old_vert),
            false => None,
        };
        self.disk_vert_replace(old_edge.0, target.0, kill_vert.0);
        self.disk_remove(kill_edge.0, target.0);
        let mut degenerate: Ring<u32> = Ring::new();
        let mut duplicates: Ring<u32> = Ring::new();
        let radial: Vec<u32> = self.edge_loops(kill_edge).map(|lp| lp.0).collect();
        for lp in radial {
            let record = self.loops.records[lp as usize];
            if self.loops.records[record.next as usize].vert == kill_vert.0 {
                self.loops.records[record.next as usize].vert = target.0;
            }
            self.loops.records[record.next as usize].prev = record.prev;
            self.loops.records[record.prev as usize].next = record.next;
            let face = &mut self.faces.records[record.face as usize];
            if face.first == lp {
                face.first = record.next;
            }
            face.len -= 1;
            if face.len < 3 {
                degenerate.push(record.face);
            } else if options.kill_duplicate_faces {
                duplicates.push(record.face);
            }
            self.release_loop(lp);
        }
        self.edges.records[kill_edge.index()].first_loop = NONE;
        self.release_edge(kill_edge.0);
        self.release_vert(kill_vert.0);
        if let Some(splice) = splice {
            self.splice_edges_raw(old_edge.0, splice.0);
        }
        self.kill_faces_after_join(&degenerate, &duplicates);
        self.touch_topology();
        Ok(old_edge)
    }

    fn kill_faces_after_join(&mut self, degenerate: &[u32], duplicates: &[u32]) {
        for &face in degenerate.iter().rev() {
            if !self.faces.is_alive(face) {
                continue;
            }
            match self.faces.records[face as usize].len {
                0 => self.release_face(face),
                _ => self.kill_face_raw(face),
            }
        }
        for &face in duplicates.iter().rev() {
            if self.faces.is_alive(face) && self.face_find_double(FaceId(face)).is_some() {
                self.kill_face_raw(face);
            }
        }
    }

    pub fn join_vert_kill_edge(
        &mut self,
        kill_edge: EdgeId,
        kill_vert: VertId,
        check_edge_exists: bool,
    ) -> Result<VertId> {
        self.check_edge(kill_edge)?;
        self.check_vert(kill_vert)?;
        let target = self
            .edge_other_vert(kill_edge, kill_vert)
            .ok_or(Error::VertNotInEdge(kill_vert, kill_edge))?;
        let mut degenerate: Ring<u32> = Ring::new();
        let mut collapsing = vec![kill_edge];
        collapsing.extend(
            self.vert_edges(kill_vert)
                .filter(|&edge| edge != kill_edge && self.edge_has_vert(edge, target)),
        );
        for edge in collapsing {
            let radial: Vec<u32> = self.edge_loops(edge).map(|lp| lp.0).collect();
            for lp in radial {
                let record = self.loops.records[lp as usize];
                if self.loops.records[record.next as usize].vert == kill_vert.0 {
                    self.loops.records[record.next as usize].vert = target.0;
                }
                self.loops.records[record.next as usize].prev = record.prev;
                self.loops.records[record.prev as usize].next = record.next;
                let face = &mut self.faces.records[record.face as usize];
                if face.first == lp {
                    face.first = record.next;
                }
                face.len -= 1;
                if face.len < 3 && !degenerate.contains(&record.face) {
                    degenerate.push(record.face);
                }
                self.release_loop(lp);
            }
            self.edges.records[edge.index()].first_loop = NONE;
            self.kill_edge_raw(edge.0);
        }
        loop {
            let edge = self.verts.records[kill_vert.index()].edge;
            if edge == NONE {
                break;
            }
            let existing = match check_edge_exists {
                true => {
                    let other = self.edge_other_vert(EdgeId(edge), kill_vert);
                    other.and_then(|other| self.edge_between(target, other))
                }
                false => None,
            };
            self.edge_vert_swap(edge, target.0, kill_vert.0);
            if let Some(existing) = existing {
                self.splice_edges_raw(existing.0, edge);
            }
        }
        self.kill_faces_after_join(&degenerate, &[]);
        self.release_vert(kill_vert.0);
        self.touch_topology();
        Ok(target)
    }

    pub fn join_faces_kill_edge(&mut self, a: FaceId, b: FaceId, edge: EdgeId) -> Result<FaceId> {
        self.check_face(a)?;
        self.check_face(b)?;
        self.check_edge(edge)?;
        if a == b || !self.is_edge_manifold(edge) {
            return Err(Error::Unjoinable);
        }
        let l_a = self
            .face_edge_share_loop(a, edge)
            .ok_or(Error::EdgeNotInFace(edge, a))?;
        let l_b = self
            .face_edge_share_loop(b, edge)
            .ok_or(Error::EdgeNotInFace(edge, b))?;
        if self.loop_vert(l_a) == self.loop_vert(l_b) {
            return Err(Error::Unjoinable);
        }
        if self.is_edge_in_face(self.loop_edge(self.loop_next(l_a)), b)
            || self.is_edge_in_face(self.loop_edge(self.loop_prev(l_a)), b)
            || self.is_edge_in_face(self.loop_edge(self.loop_next(l_b)), a)
            || self.is_edge_in_face(self.loop_edge(self.loop_prev(l_b)), a)
        {
            return Err(Error::Unjoinable);
        }
        if self.face_share_edge_count(a, b) > 1 {
            return Err(Error::Unjoinable);
        }
        let verts_a: IdSet<VertId> = self
            .face_loops(a)
            .filter(|&lp| lp != l_a)
            .map(|lp| self.loop_vert(lp))
            .collect();
        if self
            .face_loops(b)
            .any(|lp| lp != l_b && verts_a.contains(self.loop_vert(lp)))
        {
            return Err(Error::DoubleFace);
        }
        let (la, lb) = (l_a.0, l_b.0);
        let ra = self.loops.records[la as usize];
        let rb = self.loops.records[lb as usize];
        self.loops.records[ra.prev as usize].next = rb.next;
        self.loops.records[rb.next as usize].prev = ra.prev;
        self.loops.records[ra.next as usize].prev = rb.prev;
        self.loops.records[rb.prev as usize].next = ra.next;
        if self.faces.records[a.index()].first == la {
            self.faces.records[a.index()].first = ra.next;
        }
        let len = self.faces.records[a.index()].len + self.faces.records[b.index()].len - 2;
        self.faces.records[a.index()].len = len;
        let loops: Vec<u32> = self.face_loops(a).map(|lp| lp.0).collect();
        for lp in loops {
            self.loops.records[lp as usize].face = a.0;
        }
        let [v1, v2] = self.edges.records[edge.index()].verts;
        self.disk_remove(edge.0, v1);
        self.disk_remove(edge.0, v2);
        self.edges.records[edge.index()].first_loop = NONE;
        self.release_edge(edge.0);
        self.release_loop(la);
        self.release_loop(lb);
        self.release_face(b.0);
        self.touch_topology();
        Ok(a)
    }

    pub fn join_face_pair(
        &mut self,
        a: LoopId,
        b: LoopId,
        doubles: DoubleFace,
    ) -> Result<JoinedFace> {
        self.join_face_pair_impl(a, b, doubles, true)
    }

    pub fn join_face_pair_keep_shared(
        &mut self,
        a: LoopId,
        b: LoopId,
        doubles: DoubleFace,
    ) -> Result<JoinedFace> {
        self.join_face_pair_impl(a, b, doubles, false)
    }

    fn join_face_pair_impl(
        &mut self,
        a: LoopId,
        b: LoopId,
        doubles: DoubleFace,
        delete_shared: bool,
    ) -> Result<JoinedFace> {
        self.check_loop(a)?;
        self.check_loop(b)?;
        if self.loop_vert(a) == self.loop_vert(b) {
            self.reverse_face(self.loop_face(b))?;
        }
        let faces = [self.loop_face(a), self.loop_face(b)];
        self.join_faces_impl(&faces, doubles, delete_shared)
    }

    pub fn join_faces(&mut self, faces: &[FaceId], doubles: DoubleFace) -> Result<JoinedFace> {
        self.join_faces_impl(faces, doubles, true)
    }

    pub fn join_faces_keep_shared(
        &mut self,
        faces: &[FaceId],
        doubles: DoubleFace,
    ) -> Result<JoinedFace> {
        self.join_faces_impl(faces, doubles, false)
    }

    fn join_faces_impl(
        &mut self,
        faces: &[FaceId],
        doubles: DoubleFace,
        delete_shared: bool,
    ) -> Result<JoinedFace> {
        for &face in faces {
            self.check_face(face)?;
        }
        match faces {
            [] => return Err(Error::Unjoinable),
            [face] => {
                return Ok(JoinedFace {
                    face: *face,
                    double: None,
                });
            }
            _ => {}
        }
        let had_active = self.active_face.is_some();
        let joining: IdSet<FaceId> = faces.iter().copied().collect();
        let mut edges: Vec<EdgeId> = Vec::new();
        let mut kill_edges: Vec<EdgeId> = Vec::new();
        let mut kill_verts: Vec<VertId> = Vec::new();
        let mut marked_edges: IdSet<EdgeId> = IdSet::new();
        let mut marked_verts: IdSet<VertId> = IdSet::new();
        let mut start: Option<[VertId; 2]> = None;
        for &face in faces {
            for lp in self.face_loops_vec(face) {
                let edge = self.loop_edge(lp);
                let count = self
                    .edge_faces(edge)
                    .filter(|&other| joining.contains(other))
                    .count();
                if count > 2 {
                    return Err(Error::Unjoinable);
                }
                if count == 1 {
                    edges.push(edge);
                    if start.is_none() {
                        let vert = self.loop_vert(lp);
                        let other = self.edge_other_vert(edge, vert).unwrap_or(vert);
                        start = Some([vert, other]);
                    }
                } else if count == 2 {
                    let [v1, v2] = self.edge_verts(edge);
                    let d1 = self.is_vert_manifold_within(v1, &joining);
                    let d2 = self.is_vert_manifold_within(v2, &joining);
                    if !d1 && !d2 && !marked_edges.contains(edge) {
                        if delete_shared && self.edge_face_count_at_most(edge, 3) <= 2 {
                            kill_edges.push(edge);
                        }
                        marked_edges.insert(edge);
                    } else {
                        for (manifold, vert) in [(d1, v1), (d2, v2)] {
                            if manifold && marked_verts.insert(vert) && delete_shared {
                                kill_verts.push(vert);
                            }
                        }
                    }
                }
            }
        }
        let start = start.ok_or(Error::Unjoinable)?;
        let (verts, sorted) = self.sort_edge_winding(start, &edges)?;
        let mut face = self.create_face_raw(&verts, &sorted, Some(faces[0].0));
        let existing = self.face_find_double(FaceId(face));
        let mut double = None;
        if let Some(existing) = existing {
            match doubles {
                DoubleFace::Keep => double = Some(existing),
                DoubleFace::Reuse => {
                    self.kill_face_raw(face);
                    face = existing.0;
                }
            }
        }
        let reusing = existing.is_some() && doubles == DoubleFace::Reuse;
        if !reusing {
            for lp in self.face_loops_vec(FaceId(face)) {
                let mut other = self.loop_radial_next(lp);
                while other != lp && !joining.contains(self.loop_face(other)) {
                    other = self.loop_radial_next(other);
                }
                if other != lp {
                    if self.loop_vert(other) != self.loop_vert(lp) {
                        other = self.loop_next(other);
                    }
                    self.copy_loop_attrs(other.0, lp.0);
                }
            }
        }
        for edge in kill_edges {
            if self.contains_edge(edge) {
                self.kill_edge_raw(edge.0);
            }
        }
        for vert in kill_verts {
            if self.contains_vert(vert) {
                self.kill_vert_raw(vert.0);
            }
        }
        if !delete_shared {
            for &joined in faces {
                if self.contains_face(joined) {
                    self.kill_face_raw(joined.0);
                }
            }
        }
        if had_active && self.active_face.is_none() {
            self.active_face = Some(face);
        }
        self.touch_topology();
        Ok(JoinedFace {
            face: FaceId(face),
            double,
        })
    }

    fn is_vert_manifold_within(&self, vert: VertId, faces: &IdSet<FaceId>) -> bool {
        if self.is_vert_loose(vert) {
            return false;
        }
        self.vert_edges(vert).all(|edge| {
            !self.is_edge_wire(edge)
                && !self.is_edge_boundary(edge)
                && self.edge_faces(edge).all(|face| faces.contains(face))
        })
    }

    pub fn splice_verts(&mut self, target: VertId, source: VertId) -> Result<()> {
        self.check_vert(target)?;
        self.check_vert(source)?;
        if target == source {
            return Err(Error::SelfSplice);
        }
        if let Some(edge) = self.edge_between(target, source) {
            self.join_vert_kill_edge(edge, source, false)?;
            return Ok(());
        }
        loop {
            let edge = self.verts.records[source.index()].edge;
            if edge == NONE {
                break;
            }
            self.edge_vert_swap(edge, target.0, source.0);
        }
        self.release_vert(source.0);
        self.touch_topology();
        Ok(())
    }

    pub fn splice_edges(&mut self, target: EdgeId, source: EdgeId) -> Result<()> {
        self.check_edge(target)?;
        self.check_edge(source)?;
        if target == source {
            return Err(Error::SelfSplice);
        }
        let [a, b] = self.edge_verts(target);
        if !self.edge_has_vert(source, a) || !self.edge_has_vert(source, b) {
            return Err(Error::EdgesNotCoincident);
        }
        self.splice_edges_raw(target.0, source.0);
        self.touch_topology();
        Ok(())
    }

    pub(crate) fn splice_edges_raw(&mut self, target: u32, source: u32) {
        loop {
            let lp = self.edges.records[source as usize].first_loop;
            if lp == NONE {
                break;
            }
            self.radial_remove(source, lp);
            self.radial_append(target, lp);
        }
        self.kill_edge_raw(source);
    }

    #[must_use]
    pub fn splice_would_double_edge(&self, a: VertId, b: VertId) -> bool {
        let neighbours: IdSet<VertId> = self.vert_verts(a).collect();
        self.vert_verts(b).any(|vert| neighbours.contains(vert))
    }

    #[must_use]
    pub fn splice_would_double_face(&self, a: VertId, b: VertId) -> bool {
        let mut loops_a: Vec<LoopId> = self.vert_loops(a).collect();
        let mut loops_b: Vec<LoopId> = self.vert_loops(b).collect();
        if loops_a.is_empty() || loops_b.is_empty() {
            return false;
        }
        loops_a.sort_by_key(|&lp| self.face_len(self.loop_face(lp)));
        loops_b.sort_by_key(|&lp| self.face_len(self.loop_face(lp)));
        for &l_a in &loops_a {
            for &l_b in &loops_b {
                let len = self.face_len(self.loop_face(l_a));
                if len != self.face_len(self.loop_face(l_b)) {
                    continue;
                }
                let walk = |start: LoopId, forward: bool| -> Vec<VertId> {
                    let mut out = Vec::with_capacity(len - 1);
                    let mut lp = start;
                    for _ in 1..len {
                        lp = match forward {
                            true => self.loop_next(lp),
                            false => self.loop_prev(lp),
                        };
                        out.push(self.loop_vert(lp));
                    }
                    out
                };
                let ring_a = walk(l_a, true);
                if ring_a == walk(l_b, true) || ring_a == walk(l_b, false) {
                    return true;
                }
            }
        }
        false
    }
}
