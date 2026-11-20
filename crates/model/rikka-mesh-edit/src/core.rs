use glam::Vec3;
use smallvec::SmallVec;

use crate::data::KEY_INDEX_NONE;
use crate::error::{Error, Result};
use crate::geometry_set::Geometry;
use crate::ids::{EdgeId, FaceId, LoopId, NONE, VertId};
use crate::mesh::{EditMesh, Flags};

pub(crate) type Ring<T> = SmallVec<[T; 8]>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeleteContext {
    Verts,
    Edges,
    EdgesFaces,
    OnlyFaces,
    OnlyTagged,
    Faces,
}

#[must_use]
#[derive(Debug)]
pub struct NewFace<'a> {
    mesh: &'a mut EditMesh,
    verts: &'a [VertId],
    example: Option<FaceId>,
    reuse_existing: bool,
    create_edges: bool,
    match_winding: bool,
    edges_from_previous: bool,
}

impl NewFace<'_> {
    pub fn example(mut self, face: FaceId) -> Self {
        self.example = Some(face);
        self
    }

    pub fn reuse_existing(mut self) -> Self {
        self.reuse_existing = true;
        self
    }

    pub fn existing_edges(mut self) -> Self {
        self.create_edges = false;
        self
    }

    pub fn match_winding(mut self) -> Self {
        self.match_winding = true;
        self
    }

    pub fn edges_from_previous(mut self) -> Self {
        self.edges_from_previous = true;
        self
    }

    pub fn create(self) -> Result<FaceId> {
        let NewFace {
            mesh,
            verts,
            example,
            reuse_existing,
            create_edges,
            match_winding,
            edges_from_previous,
        } = self;
        mesh.check_face_verts(verts)?;
        if let Some(face) = example {
            mesh.check_face(face)?;
        }
        if reuse_existing && let Some(existing) = mesh.face_exists(verts) {
            return Ok(existing);
        }
        let count = verts.len();
        let mut pairs: Vec<[u32; 2]> = (0..count)
            .map(|index| {
                let (a, b) = (verts[index].0, verts[(index + 1) % count].0);
                [a.min(b), a.max(b)]
            })
            .collect();
        pairs.sort_unstable();
        if pairs.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::RepeatedEdge);
        }
        if !create_edges {
            for index in 0..count {
                let (a, b) = (verts[index], verts[(index + 1) % count]);
                if mesh.edge_between(a, b).is_none() {
                    return Err(Error::MissingEdge(a, b));
                }
            }
        }
        if create_edges && edges_from_previous {
            for index in 0..count {
                let (previous, vert) = (verts[(index + count - 1) % count], verts[index]);
                if mesh.edge_between(previous, vert).is_none() {
                    mesh.alloc_edge(previous.0, vert.0);
                }
            }
        }
        let mut edges: Ring<u32> = SmallVec::with_capacity(count);
        for index in 0..count {
            let (a, b) = (verts[index], verts[(index + 1) % count]);
            edges.push(match mesh.edge_between(a, b) {
                Some(edge) => edge.0,
                None => mesh.alloc_edge(a.0, b.0),
            });
        }
        let mut order: Ring<u32> = verts.iter().map(|vert| vert.0).collect();
        if match_winding {
            match mesh.prefers_reversed(&order, &edges) {
                true => {
                    order.reverse();
                    order.rotate_right(1);
                    edges.reverse();
                }
                false => {
                    order.rotate_right(1);
                    edges.rotate_right(1);
                }
            }
        }
        let face = mesh.alloc_face();
        mesh.link_face_loops(face, &order, &edges);
        if let Some(example) = example {
            mesh.copy_face_attrs(example.0, face);
        }
        mesh.touch_topology();
        Ok(FaceId(face))
    }
}

impl EditMesh {
    pub(crate) fn check_vert(&self, vert: VertId) -> Result<()> {
        match self.contains_vert(vert) {
            true => Ok(()),
            false => Err(Error::DeadVert(vert)),
        }
    }

    pub(crate) fn check_edge(&self, edge: EdgeId) -> Result<()> {
        match self.contains_edge(edge) {
            true => Ok(()),
            false => Err(Error::DeadEdge(edge)),
        }
    }

    pub(crate) fn check_loop(&self, lp: LoopId) -> Result<()> {
        match self.contains_loop(lp) {
            true => Ok(()),
            false => Err(Error::DeadLoop(lp)),
        }
    }

    pub(crate) fn check_face(&self, face: FaceId) -> Result<()> {
        match self.contains_face(face) {
            true => Ok(()),
            false => Err(Error::DeadFace(face)),
        }
    }

    fn check_face_verts(&self, verts: &[VertId]) -> Result<()> {
        if verts.len() < 3 {
            return Err(Error::TooFewCorners);
        }
        for (index, &vert) in verts.iter().enumerate() {
            self.check_vert(vert)?;
            if verts[(index + 1) % verts.len()] == vert {
                return Err(Error::DegenerateEdge);
            }
        }
        Ok(())
    }

    fn prefers_reversed(&self, verts: &[u32], edges: &[u32]) -> bool {
        let mut winding = [0u32; 2];
        for (&vert, &edge) in verts.iter().zip(edges) {
            let first = self.edges.records[edge as usize].first_loop;
            if first == NONE {
                continue;
            }
            winding[usize::from(self.loops.records[first as usize].vert == vert)] += 1;
        }
        winding[0] < winding[1]
    }

    pub fn add_vert(&mut self, position: Vec3) -> VertId {
        let vert = self.alloc_vert(position);
        self.touch_topology();
        VertId(vert)
    }

    pub fn add_vert_like(&mut self, position: Vec3, example: VertId) -> Result<VertId> {
        self.check_vert(example)?;
        let vert = self.alloc_vert(position);
        self.copy_vert_attrs(example.0, vert);
        self.vert_data.key_index[vert as usize] = KEY_INDEX_NONE;
        self.touch_topology();
        Ok(VertId(vert))
    }

    pub fn add_edge(&mut self, a: VertId, b: VertId) -> Result<EdgeId> {
        self.check_vert(a)?;
        self.check_vert(b)?;
        if a == b {
            return Err(Error::DegenerateEdge);
        }
        let edge = self.alloc_edge(a.0, b.0);
        self.touch_topology();
        Ok(EdgeId(edge))
    }

    pub fn add_edge_like(&mut self, a: VertId, b: VertId, example: EdgeId) -> Result<EdgeId> {
        self.check_edge(example)?;
        let edge = self.add_edge(a, b)?;
        self.copy_edge_attrs(example.0, edge.0);
        Ok(edge)
    }

    pub fn ensure_edge(&mut self, a: VertId, b: VertId) -> Result<EdgeId> {
        self.check_vert(a)?;
        self.check_vert(b)?;
        match self.edge_between(a, b) {
            Some(edge) => Ok(edge),
            None => self.add_edge(a, b),
        }
    }

    pub fn new_face<'a>(&'a mut self, verts: &'a [VertId]) -> NewFace<'a> {
        NewFace {
            mesh: self,
            verts,
            example: None,
            reuse_existing: false,
            create_edges: true,
            match_winding: false,
            edges_from_previous: false,
        }
    }

    pub fn add_face(&mut self, verts: &[VertId]) -> Result<FaceId> {
        self.new_face(verts).create()
    }

    pub fn add_face_from_edges(
        &mut self,
        start: [VertId; 2],
        edges: &[EdgeId],
        example: Option<FaceId>,
    ) -> Result<FaceId> {
        for &edge in edges {
            self.check_edge(edge)?;
        }
        if edges.len() < 3 {
            return Err(Error::TooFewCorners);
        }
        if let Some(example) = example {
            self.check_face(example)?;
        }
        let (verts, sorted) = self.sort_edge_winding(start, edges)?;
        Ok(FaceId(self.create_face_raw(
            &verts,
            &sorted,
            example.map(|face| face.0),
        )))
    }

    pub(crate) fn create_face_raw(
        &mut self,
        verts: &[u32],
        edges: &[u32],
        example: Option<u32>,
    ) -> u32 {
        let face = self.alloc_face();
        self.link_face_loops(face, verts, edges);
        if let Some(example) = example {
            self.copy_face_attrs(example, face);
        }
        self.touch_topology();
        face
    }

    pub(crate) fn sort_edge_winding(
        &self,
        start: [VertId; 2],
        edges: &[EdgeId],
    ) -> Result<(Ring<u32>, Ring<u32>)> {
        let len = edges.len();
        let mut pending: Ring<u32> = edges.iter().map(|edge| edge.0).collect();
        let mut verts: Ring<u32> = SmallVec::with_capacity(len);
        let mut sorted: Ring<u32> = SmallVec::with_capacity(len);
        let first = pending
            .iter()
            .position(|&edge| {
                let pair = self.edges.records[edge as usize].verts;
                pair.contains(&start[0].0) && pair.contains(&start[1].0)
            })
            .ok_or(Error::OpenBoundary)?;
        let mut vert = start[0].0;
        let mut edge = pending.swap_remove(first);
        loop {
            verts.push(vert);
            sorted.push(edge);
            let pair = self.edges.records[edge as usize].verts;
            vert = if pair[0] == vert { pair[1] } else { pair[0] };
            if sorted.len() == len {
                break;
            }
            let next = pending
                .iter()
                .position(|&candidate| self.edges.records[candidate as usize].verts.contains(&vert))
                .ok_or(Error::OpenBoundary)?;
            edge = pending.swap_remove(next);
        }
        if vert != verts[0] {
            return Err(Error::OpenBoundary);
        }
        let mut unique = verts.clone();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != verts.len() {
            return Err(Error::OpenBoundary);
        }
        Ok((verts, sorted))
    }

    #[must_use]
    pub fn edge_between(&self, a: VertId, b: VertId) -> Option<EdgeId> {
        if !self.contains_vert(a) || !self.contains_vert(b) || a == b {
            return None;
        }
        let (a, b) = match self.vert_edge_count_at_most(a, usize::MAX)
            <= self.vert_edge_count_at_most(b, usize::MAX)
        {
            true => (a, b),
            false => (b, a),
        };
        self.vert_edges(a)
            .find(|&edge| self.edges.records[edge.index()].verts.contains(&b.0))
    }

    #[must_use]
    pub fn face_exists(&self, verts: &[VertId]) -> Option<FaceId> {
        let &first = verts.first()?;
        if !verts.iter().all(|&vert| self.contains_vert(vert)) {
            return None;
        }
        let count = verts.len();
        for lp in self.vert_loops(first) {
            let face = self.loop_face(lp);
            if self.face_len(face) != count {
                continue;
            }
            let forward = (0..count)
                .all(|step| self.loop_vert(self.loop_step(lp, step as isize)) == verts[step]);
            let backward = (0..count)
                .all(|step| self.loop_vert(self.loop_step(lp, -(step as isize))) == verts[step]);
            if forward || backward {
                return Some(face);
            }
        }
        None
    }

    #[must_use]
    pub fn face_find_double(&self, face: FaceId) -> Option<FaceId> {
        let first = self.face_first_loop(face);
        let len = self.face_len(face);
        let mut radial = self.loop_radial_next(first);
        while radial != first {
            let other = self.loop_face(radial);
            if self.face_len(other) == len {
                let forward = self.loop_vert(radial) == self.loop_vert(first);
                let mut a = first;
                let mut b = radial;
                let mut matched = true;
                for _ in 0..len {
                    if self.loop_edge(a) != self.loop_edge(b) {
                        matched = false;
                        break;
                    }
                    a = match forward {
                        true => self.loop_next(a),
                        false => self.loop_prev(a),
                    };
                    b = self.loop_next(b);
                }
                if matched {
                    return Some(other);
                }
            }
            radial = self.loop_radial_next(radial);
        }
        None
    }

    pub(crate) fn loop_step(&self, lp: LoopId, steps: isize) -> LoopId {
        let mut current = lp;
        match steps >= 0 {
            true => (0..steps).for_each(|_| current = self.loop_next(current)),
            false => (0..-steps).for_each(|_| current = self.loop_prev(current)),
        }
        current
    }

    pub fn copy_face(
        &mut self,
        face: FaceId,
        copy_verts: bool,
        copy_edges: bool,
    ) -> Result<FaceId> {
        self.check_face(face)?;
        let loops = self.face_loops_vec(face);
        let count = loops.len();
        let mut verts: Ring<u32> = SmallVec::with_capacity(count);
        for &lp in &loops {
            let vert = self.loops.records[lp.index()].vert;
            verts.push(match copy_verts {
                true => {
                    let copy = self.alloc_vert(self.positions[vert as usize]);
                    self.copy_vert_attrs(vert, copy);
                    self.vert_data.key_index[copy as usize] = KEY_INDEX_NONE;
                    copy
                }
                false => vert,
            });
        }
        let mut edges: Ring<u32> = SmallVec::with_capacity(count);
        for (index, &lp) in loops.iter().enumerate() {
            let edge = self.loops.records[lp.index()].edge;
            edges.push(match copy_edges {
                true => {
                    let (here, next) = (verts[index], verts[(index + 1) % count]);
                    let pair = match self.edges.records[edge as usize].verts[0]
                        == self.loops.records[lp.index()].vert
                    {
                        true => (here, next),
                        false => (next, here),
                    };
                    let copy = self.alloc_edge(pair.0, pair.1);
                    self.copy_edge_attrs(edge, copy);
                    copy
                }
                false => edge,
            });
        }
        let copy = self.alloc_face();
        self.link_face_loops(copy, &verts, &edges);
        self.copy_face_attrs(face.0, copy);
        let copied = self.face_loops_vec(FaceId(copy));
        for (&from, &to) in loops.iter().zip(&copied) {
            self.copy_loop_attrs(from.0, to.0);
        }
        self.touch_topology();
        Ok(FaceId(copy))
    }

    pub fn kill_face(&mut self, face: FaceId) -> Result<()> {
        self.check_face(face)?;
        self.kill_face_raw(face.0);
        self.touch_topology();
        Ok(())
    }

    pub(crate) fn kill_face_raw(&mut self, face: u32) {
        let first = self.faces.records[face as usize].first;
        let mut lp = first;
        loop {
            let next = self.loops.records[lp as usize].next;
            let edge = self.loops.records[lp as usize].edge;
            self.radial_remove(edge, lp);
            self.release_loop(lp);
            lp = next;
            if lp == first {
                break;
            }
        }
        self.release_face(face);
    }

    pub fn kill_face_loose(&mut self, face: FaceId) -> Result<()> {
        self.check_face(face)?;
        let first = self.faces.records[face.index()].first;
        let mut lp = first;
        loop {
            let next = self.loops.records[lp as usize].next;
            let edge = self.loops.records[lp as usize].edge;
            self.radial_remove(edge, lp);
            self.release_loop(lp);
            if self.edges.records[edge as usize].first_loop == NONE {
                let [a, b] = self.edges.records[edge as usize].verts;
                self.disk_remove(edge, a);
                self.disk_remove(edge, b);
                self.release_edge(edge);
                for vert in [a, b] {
                    if self.verts.records[vert as usize].edge == NONE {
                        self.release_vert(vert);
                    }
                }
            }
            lp = next;
            if lp == first {
                break;
            }
        }
        self.release_face(face.0);
        self.touch_topology();
        Ok(())
    }

    pub fn kill_edge(&mut self, edge: EdgeId) -> Result<()> {
        self.check_edge(edge)?;
        self.kill_edge_raw(edge.0);
        self.touch_topology();
        Ok(())
    }

    pub(crate) fn kill_edge_raw(&mut self, edge: u32) {
        loop {
            let lp = self.edges.records[edge as usize].first_loop;
            if lp == NONE {
                break;
            }
            let face = self.loops.records[lp as usize].face;
            self.kill_face_raw(face);
        }
        let [a, b] = self.edges.records[edge as usize].verts;
        self.disk_remove(edge, a);
        self.disk_remove(edge, b);
        self.release_edge(edge);
    }

    pub fn kill_vert(&mut self, vert: VertId) -> Result<()> {
        self.check_vert(vert)?;
        self.kill_vert_raw(vert.0);
        self.touch_topology();
        Ok(())
    }

    pub(crate) fn kill_vert_raw(&mut self, vert: u32) {
        loop {
            let edge = self.verts.records[vert as usize].edge;
            if edge == NONE {
                break;
            }
            self.kill_edge_raw(edge);
        }
        self.release_vert(vert);
    }

    pub fn kill_face_edges(&mut self, face: FaceId) -> Result<()> {
        self.check_face(face)?;
        for edge in self.face_edges_vec(face) {
            if self.contains_edge(edge) {
                self.kill_edge_raw(edge.0);
            }
        }
        self.touch_topology();
        Ok(())
    }

    pub fn kill_face_verts(&mut self, face: FaceId) -> Result<()> {
        self.check_face(face)?;
        for vert in self.face_verts_vec(face) {
            if self.contains_vert(vert) {
                self.kill_vert_raw(vert.0);
            }
        }
        self.touch_topology();
        Ok(())
    }

    pub fn delete(&mut self, context: DeleteContext, geometry: &Geometry) {
        let mut verts = geometry.verts.clone();
        let mut edges = geometry.edges.clone();
        let faces = geometry.faces.clone();
        match context {
            DeleteContext::Verts => self.kill_tagged(None, None, Some(&verts)),
            DeleteContext::Edges => {
                for edge in edges.iter() {
                    verts.extend(self.edge_verts(edge));
                }
                self.kill_tagged(None, Some(&edges), None);
                for vert in verts.iter() {
                    if self.contains_vert(vert) && self.vert_first_edge(vert).is_none() {
                        self.kill_vert_raw(vert.0);
                    }
                }
            }
            DeleteContext::EdgesFaces => self.kill_tagged(None, Some(&edges), None),
            DeleteContext::OnlyFaces => self.kill_tagged(Some(&faces), None, None),
            DeleteContext::OnlyTagged => self.kill_tagged(Some(&faces), Some(&edges), Some(&verts)),
            DeleteContext::Faces => {
                for face in faces.iter() {
                    for lp in self.face_loops(face) {
                        verts.insert(self.loop_vert(lp));
                        edges.insert(self.loop_edge(lp));
                    }
                }
                let kept: Vec<FaceId> =
                    self.faces().filter(|face| !faces.contains(*face)).collect();
                for face in kept {
                    for lp in self.face_loops_vec(face) {
                        verts.remove(self.loop_vert(lp));
                        edges.remove(self.loop_edge(lp));
                    }
                }
                let kept_edges: Vec<EdgeId> =
                    self.edges().filter(|edge| !edges.contains(*edge)).collect();
                for edge in kept_edges {
                    for vert in self.edge_verts(edge) {
                        verts.remove(vert);
                    }
                }
                self.kill_tagged(Some(&faces), Some(&edges), Some(&verts));
            }
        }
        self.touch_topology();
    }

    fn kill_tagged(
        &mut self,
        faces: Option<&crate::IdSet<FaceId>>,
        edges: Option<&crate::IdSet<EdgeId>>,
        verts: Option<&crate::IdSet<VertId>>,
    ) {
        if let Some(faces) = faces {
            for face in faces.iter() {
                if self.contains_face(face) {
                    self.kill_face_raw(face.0);
                }
            }
        }
        if let Some(edges) = edges {
            for edge in edges.iter() {
                if self.contains_edge(edge) {
                    self.kill_edge_raw(edge.0);
                }
            }
        }
        if let Some(verts) = verts {
            for vert in verts.iter() {
                if self.contains_vert(vert) {
                    self.kill_vert_raw(vert.0);
                }
            }
        }
    }

    pub(crate) fn copy_vert_attrs(&mut self, from: u32, to: u32) {
        if from == to {
            return;
        }
        self.vert_data.copy(from as usize, to as usize);
        let source = self.verts.records[from as usize].flags;
        let target = self.verts.records[to as usize].flags;
        self.put_vert_flags(
            VertId(to),
            (target & Flags::SELECT) | (source - Flags::SELECT),
        );
        self.vert_normals[to as usize] = self.vert_normals[from as usize];
    }

    pub(crate) fn copy_edge_attrs(&mut self, from: u32, to: u32) {
        if from == to {
            return;
        }
        self.edge_data.copy(from as usize, to as usize);
        let source = self.edges.records[from as usize].flags;
        let target = self.edges.records[to as usize].flags;
        self.put_edge_flags(
            EdgeId(to),
            (target & Flags::SELECT) | (source - Flags::SELECT),
        );
    }

    pub(crate) fn copy_face_attrs(&mut self, from: u32, to: u32) {
        if from == to {
            return;
        }
        self.face_data.copy(from as usize, to as usize);
        let source = self.faces.records[from as usize];
        let target = self.faces.records[to as usize].flags;
        self.put_face_flags(
            FaceId(to),
            (target & Flags::SELECT) | (source.flags - Flags::SELECT),
        );
        self.faces.records[to as usize].material = source.material;
        self.face_normals[to as usize] = self.face_normals[from as usize];
    }

    pub(crate) fn copy_loop_attrs(&mut self, from: u32, to: u32) {
        if from == to {
            return;
        }
        self.loop_data.copy(from as usize, to as usize);
        let source = self.loops.records[from as usize].flags;
        let target = &mut self.loops.records[to as usize].flags;
        *target = (*target & Flags::SELECT) | (source - Flags::SELECT);
    }
}
