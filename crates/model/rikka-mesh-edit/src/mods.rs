use bitflags::bitflags;
use glam::Vec3;
use rikka_polymesh::{NgonMethod, Polymesh, QuadMethod};

use crate::core::Ring;
use crate::data::KEY_INDEX_NONE;
use crate::error::{Error, Result};
use crate::geometry_set::Geometry;
use crate::ids::{EdgeId, FaceId, LoopId, NONE, VertId};
use crate::join::{DoubleFace, JoinOptions};
use crate::math::{quad_flip_flags, rotate_beauty_area};
use crate::mesh::EditMesh;
use crate::sets::{IdMap, IdSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rotation {
    #[default]
    Clockwise,
    CounterClockwise,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct RotateChecks: u8 {
        const EXISTS = 1;
        const SPLICE = 1 << 1;
        const DEGENERATE = 1 << 2;
        const BEAUTY = 1 << 3;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Triangulated {
    pub faces: Vec<FaceId>,
    pub edges: Vec<EdgeId>,
    pub doubles: Vec<FaceId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Duplicate {
    pub verts: IdMap<VertId, VertId>,
    pub edges: IdMap<EdgeId, EdgeId>,
    pub faces: IdMap<FaceId, FaceId>,
}

impl EditMesh {
    pub fn reverse_face(&mut self, face: FaceId) -> Result<()> {
        self.check_face(face)?;
        let loops = self.face_loops_vec(face);
        let count = loops.len();
        let edges: Ring<u32> = loops
            .iter()
            .map(|lp| self.loops.records[lp.index()].edge)
            .collect();
        let radial_next: Ring<u32> = loops
            .iter()
            .map(|lp| self.loops.records[lp.index()].radial_next)
            .collect();
        let radial_prev: Ring<u32> = loops
            .iter()
            .map(|lp| self.loops.records[lp.index()].radial_prev)
            .collect();
        let position = |lp: u32| loops.iter().position(|other| other.0 == lp);
        let substitute = |lp: u32| match position(lp) {
            Some(index) => loops[(index + 1) % count].0,
            None => lp,
        };
        for index in 0..count {
            let occupant = loops[(index + 1) % count].0;
            let next = radial_next[index];
            let prev = radial_prev[index];
            let record = &mut self.loops.records[occupant as usize];
            record.edge = edges[index];
            record.radial_next = substitute(next);
            record.radial_prev = substitute(prev);
            if position(next).is_none() {
                self.loops.records[next as usize].radial_prev = occupant;
            }
            if position(prev).is_none() {
                self.loops.records[prev as usize].radial_next = occupant;
            }
            if self.edges.records[edges[index] as usize].first_loop == loops[index].0 {
                self.edges.records[edges[index] as usize].first_loop = occupant;
            }
        }
        for lp in &loops {
            let record = &mut self.loops.records[lp.index()];
            std::mem::swap(&mut record.next, &mut record.prev);
        }
        self.face_normals[face.index()] = -self.face_normals[face.index()];
        self.touch_topology();
        Ok(())
    }

    pub fn flip_faces(&mut self, faces: impl IntoIterator<Item = FaceId>) -> Result<()> {
        for face in faces {
            self.reverse_face(face)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn can_rotate_edge(&self, edge: EdgeId) -> bool {
        let Some((a, b)) = self.edge_face_pair(edge) else {
            return false;
        };
        let [v1, v2] = self.edge_verts(edge);
        let check = |previous: VertId, vert: VertId| {
            let first = self.face_other_vert_loop(a, previous, vert);
            let second = self.face_other_vert_loop(b, previous, vert);
            match (first, second) {
                (Some(first), Some(second)) => self.loop_vert(first) != self.loop_vert(second),
                _ => false,
            }
        };
        check(v2, v1) && check(v1, v2)
    }

    #[must_use]
    pub fn edge_rotate_loops(&self, edge: EdgeId, rotation: Rotation) -> Option<(LoopId, LoopId)> {
        let (mut a, mut b) = self.edge_face_pair(edge)?;
        let [v1, v2] = self.edge_ordered_verts(edge);
        if rotation == Rotation::Clockwise {
            std::mem::swap(&mut a, &mut b);
        }
        let first = self.face_other_vert_loop(b, v2, v1)?;
        let second = self.face_other_vert_loop(a, v1, v2)?;
        (self.loop_vert(first) != self.loop_vert(second)).then_some((first, second))
    }

    fn rotate_is_degenerate(&self, edge: EdgeId, first: LoopId, second: LoopId) -> bool {
        let [old_1, old_2] = self.edge_ordered_verts(edge);
        let v1 = self.loop_vert(first);
        let v2 = self.loop_vert(second);
        let (Some(alt_1), Some(alt_2)) = (
            self.face_other_vert_loop(self.loop_face(first), old_1, v1),
            self.face_other_vert_loop(self.loop_face(second), old_2, v2),
        ) else {
            return true;
        };
        let at = |vert: VertId| self.position(vert);
        let norm = crate::math::normalize;
        let old_dir = norm(at(old_1) - at(old_2));
        let new_dir = norm(at(v1) - at(v2));
        let old_v1 = norm(at(old_1) - at(v1));
        let old_v2 = norm(at(old_2) - at(v2));
        let new_v1 = norm(at(v1) - at(self.loop_vert(alt_1)));
        let new_v2 = norm(at(v2) - at(self.loop_vert(alt_2)));
        if old_dir.cross(old_v1).dot(new_dir.cross(new_v1)) < 0.0 {
            return true;
        }
        if old_dir.cross(old_v2).dot(new_dir.cross(new_v2)) < 0.0 {
            return true;
        }
        new_dir.dot(new_v1) > 0.999 || (-new_dir).dot(new_v2) > 0.999
    }

    pub fn rotate_edge(
        &mut self,
        edge: EdgeId,
        rotation: Rotation,
        checks: RotateChecks,
    ) -> Result<EdgeId> {
        self.check_edge(edge)?;
        if !self.can_rotate_edge(edge) {
            return Err(Error::Unrotatable(edge));
        }
        let (first, second) = self
            .edge_rotate_loops(edge, rotation)
            .ok_or(Error::Unrotatable(edge))?;
        let v1 = self.loop_vert(first);
        let v2 = self.loop_vert(second);
        let [e1, e2] = self.edge_verts(edge);
        if checks.contains(RotateChecks::BEAUTY)
            && self.position(e1).distance_squared(self.position(e2))
                <= self.position(v1).distance_squared(self.position(v2))
        {
            return Err(Error::Unrotatable(edge));
        }
        if checks.contains(RotateChecks::EXISTS) && self.edge_between(v1, v2).is_some() {
            return Err(Error::Unrotatable(edge));
        }
        if checks.contains(RotateChecks::DEGENERATE)
            && self.rotate_is_degenerate(edge, first, second)
        {
            return Err(Error::Unrotatable(edge));
        }
        let new_edge = match (
            checks.contains(RotateChecks::SPLICE),
            self.edge_between(v1, v2),
        ) {
            (true, Some(existing)) => existing,
            _ => {
                let created = self.alloc_edge(v1.0, v2.0);
                self.copy_edge_attrs(edge.0, created);
                EdgeId(created)
            }
        };
        let face_first = self.loop_face(first);
        let face_second = self.loop_face(second);
        let flags_first = self.faces.records[face_first.index()].flags;
        let flags_second = self.faces.records[face_second.index()].flags;
        let active = match self.active_face {
            Some(active) if active == face_first.0 => 1,
            Some(active) if active == face_second.0 => 2,
            _ => 0,
        };
        let flipped = !self.is_edge_contiguous(edge);
        let loop_a = self
            .face_edge_share_loop(face_first, edge)
            .ok_or(Error::EdgeNotInFace(edge, face_first))?;
        let loop_b = self
            .face_edge_share_loop(face_second, edge)
            .ok_or(Error::EdgeNotInFace(edge, face_second))?;
        let joined = self.join_face_pair(loop_a, loop_b, DoubleFace::Reuse)?.face;
        let (Some(l1), Some(l2)) = (
            self.face_vert_share_loop(joined, v1),
            self.face_vert_share_loop(joined, v2),
        ) else {
            return Err(Error::DoubleFace);
        };
        self.split_face(joined, l1, l2, None, true)?;
        let rotated = self.edge_between(v1, v2).unwrap_or(new_edge);
        if let Some((fa, fb)) = self.edge_face_pair(rotated) {
            self.faces.records[fa.index()].flags = flags_first;
            self.faces.records[fb.index()].flags = flags_second;
            match active {
                1 => self.active_face = Some(fa.0),
                2 => self.active_face = Some(fb.0),
                _ => {}
            }
            if flipped {
                self.reverse_face(fb)?;
                if rotation == Rotation::CounterClockwise {
                    let lp = self.edges.records[rotated.index()].first_loop;
                    self.edges.records[rotated.index()].first_loop =
                        self.loops.records[lp as usize].radial_next;
                }
            }
        }
        self.touch_topology();
        Ok(rotated)
    }

    pub fn collapse_vert_faces(
        &mut self,
        kill_edge: EdgeId,
        kill_vert: VertId,
        factor: f32,
        join_faces: bool,
        kill_duplicate_faces: bool,
    ) -> Result<EdgeId> {
        self.check_edge(kill_edge)?;
        self.check_vert(kill_vert)?;
        let target = self
            .edge_other_vert(kill_edge, kill_vert)
            .ok_or(Error::VertNotInEdge(kill_vert, kill_edge))?;
        if self.vert_edge_count_at_most(kill_vert, 3) > 2 {
            return Err(Error::NotTwoEdges(kill_vert));
        }
        let radial: Vec<LoopId> = self.edge_loops(kill_edge).collect();
        for lp in radial {
            if self.loop_vert(lp) == target && self.loop_next_vert(lp) == kill_vert {
                let kept = self.loop_next(lp);
                self.loop_data
                    .mix(&[(kept.0, 1.0 - factor), (lp.0, factor)], kept.index());
            }
        }
        self.mix_pair(
            crate::Domain::Point,
            kill_vert.0,
            target.0,
            kill_vert.0,
            factor,
        );
        let other_edge = self.disk_next(kill_edge, kill_vert);
        let other = self.edge_other_vert(other_edge, kill_vert);
        if join_faces {
            let faces: Vec<FaceId> = self.vert_faces(kill_vert).collect();
            let mut created = None;
            if faces.len() >= 2 {
                let joined = self.join_faces(&faces, DoubleFace::Reuse)?;
                if let (Some(other), Some(l_a)) =
                    (other, self.face_vert_share_loop(joined.face, target))
                    && let Some(l_b) = self.face_vert_share_loop(joined.face, other)
                    && let Ok(split) = self.split_face(joined.face, l_a, l_b, None, false)
                {
                    created = Some(self.loop_edge(split.lp));
                }
            }
            return created.ok_or(Error::Undissolvable(kill_vert));
        }
        self.join_edge_kill_vert(
            kill_edge,
            kill_vert,
            JoinOptions {
                check_edge_exists: true,
                kill_duplicate_faces,
            },
        )
    }

    pub fn collapse_vert_edge(
        &mut self,
        kill_edge: EdgeId,
        kill_vert: VertId,
        kill_duplicate_faces: bool,
    ) -> Result<EdgeId> {
        self.check_edge(kill_edge)?;
        self.check_vert(kill_vert)?;
        if self.edge_other_vert(kill_edge, kill_vert).is_none() {
            return Err(Error::VertNotInEdge(kill_vert, kill_edge));
        }
        self.collapse_vert_faces(kill_edge, kill_vert, 1.0, false, kill_duplicate_faces)
    }

    pub fn collapse_edge(&mut self, edge: EdgeId, kill_vert: VertId) -> Result<VertId> {
        self.join_vert_kill_edge(edge, kill_vert, true)
    }

    pub fn collapse_edge_at(&mut self, edge: EdgeId, factor: f32) -> Result<VertId> {
        self.check_edge(edge)?;
        let [keep, kill] = self.edge_verts(edge);
        let position = self.position(keep).lerp(self.position(kill), factor);
        let radial: Vec<LoopId> = self.edge_loops(edge).collect();
        for lp in radial {
            let survivor = self.loop_next(lp);
            let (keep_corner, kill_corner) = match self.loop_vert(lp) == keep {
                true => (lp, survivor),
                false => (survivor, lp),
            };
            self.loop_data.mix(
                &[(keep_corner.0, 1.0 - factor), (kill_corner.0, factor)],
                survivor.index(),
            );
        }
        self.mix_pair(crate::Domain::Point, keep.0, kill.0, keep.0, factor);
        let target = self.join_vert_kill_edge(edge, kill, true)?;
        self.positions[target.index()] = position;
        self.touch_shape();
        Ok(target)
    }

    pub fn dissolve_vert(&mut self, vert: VertId) -> Result<()> {
        self.check_vert(vert)?;
        let len = self.vert_edge_count_at_most(vert, 3);
        if len == 1 {
            return self.kill_vert(vert);
        }
        if !self.is_vert_manifold(vert) {
            let Some(edge) = self.vert_first_edge(vert) else {
                return self.kill_vert(vert);
            };
            if self.is_edge_wire(edge) && len == 2 {
                self.collapse_vert_edge(edge, vert, true)?;
                return Ok(());
            }
            return Err(Error::Undissolvable(vert));
        }
        if len == 2 && self.vert_face_count_at_most(vert, 2) == 1 {
            let edge = self
                .vert_first_edge(vert)
                .ok_or(Error::Undissolvable(vert))?;
            self.collapse_vert_edge(edge, vert, true)?;
            return Ok(());
        }
        self.dissolve_disk(vert)
    }

    pub fn dissolve_disk(&mut self, vert: VertId) -> Result<()> {
        self.check_vert(vert)?;
        if !self.is_vert_manifold(vert) {
            return Err(Error::NonManifoldVert(vert));
        }
        let failed = || Error::Undissolvable(vert);
        let mut keep_edge = None;
        let mut base_edge = None;
        let mut len = 0;
        let first = self.vert_first_edge(vert).ok_or_else(failed)?;
        let mut edge = first;
        loop {
            edge = self.disk_next(edge, vert);
            if self.edge_share_face(edge, first).is_none() {
                keep_edge = Some(edge);
                base_edge = Some(first);
                break;
            }
            len += 1;
            if edge == first {
                break;
            }
        }
        if keep_edge.is_none() && len == 3 {
            let lp = self.edge_first_loop(edge).ok_or_else(failed)?;
            self.join_face_pair(lp, self.loop_radial_next(lp), DoubleFace::Reuse)?;
            let first = self.vert_first_edge(vert).ok_or_else(failed)?;
            self.collapse_vert_faces(first, vert, 1.0, false, true)?;
            return Ok(());
        }
        if keep_edge.is_none() && len == 2 {
            let first = self.vert_first_edge(vert).ok_or_else(failed)?;
            let edge = self.collapse_vert_faces(first, vert, 1.0, true, true)?;
            if let Some(lp) = self.edge_first_loop(edge)
                && self.loop_radial_next(lp) != lp
            {
                self.join_face_pair(lp, self.loop_radial_next(lp), DoubleFace::Reuse)?;
            }
            return Ok(());
        }
        if let (Some(keep_edge), Some(base_edge)) = (keep_edge, base_edge) {
            loop {
                let mut joined = false;
                let edges: Vec<EdgeId> = self.vert_edges(vert).collect();
                for edge in edges {
                    if self.is_edge_manifold(edge) && edge != base_edge && edge != keep_edge {
                        let lp = self.edge_first_loop(edge).ok_or_else(failed)?;
                        self.join_face_pair(lp, self.loop_radial_next(lp), DoubleFace::Reuse)?;
                        joined = true;
                        break;
                    }
                }
                if !joined {
                    break;
                }
            }
            let boundary = self.is_edge_boundary(base_edge);
            let edge = self.collapse_vert_faces(base_edge, vert, 1.0, !boundary, true)?;
            if let Some(lp) = self.edge_first_loop(edge)
                && self.loop_radial_next(lp) != lp
            {
                self.join_face_pair(lp, self.loop_radial_next(lp), DoubleFace::Reuse)?;
            }
        }
        Ok(())
    }

    pub fn triangulate_face(
        &mut self,
        face: FaceId,
        quad: QuadMethod,
        ngon: NgonMethod,
    ) -> Result<Triangulated> {
        self.check_face(face)?;
        let loops = self.face_loops_vec(face);
        let count = loops.len();
        if count <= 3 {
            return Ok(Triangulated::default());
        }
        let positions: Vec<Vec3> = loops
            .iter()
            .map(|&lp| self.position(self.loop_vert(lp)))
            .collect();
        let triangles: Vec<[LoopId; 3]> = match count {
            4 => {
                let order: [usize; 4] = match quad_diagonal_02(&positions, quad) {
                    true => [0, 1, 2, 3],
                    false => [1, 2, 3, 0],
                };
                let at = |slot: usize| loops[order[slot]];
                vec![[at(0), at(1), at(2)], [at(0), at(2), at(3)]]
            }
            _ => {
                let probe = Polymesh {
                    positions,
                    face_offsets: vec![0, count as u32],
                    corner_verts: (0..count as u32).collect(),
                    ..Polymesh::default()
                };
                probe
                    .triangulate_face_with(0, quad, ngon)
                    .iter()
                    .map(|triangle| triangle.map(|slot| loops[slot as usize]))
                    .collect()
            }
        };
        let mut result = Triangulated::default();
        let last = triangles.len() - 1;
        let mut last_face = face.0;
        for (index, triangle) in triangles.iter().enumerate() {
            let verts: [u32; 3] = triangle.map(|lp| self.loops.records[lp.index()].vert);
            let mut edges = [0u32; 3];
            for slot in 0..3 {
                let (a, b) = (verts[slot], verts[(slot + 1) % 3]);
                edges[slot] = match self.edge_between(VertId(a), VertId(b)) {
                    Some(edge) => edge.0,
                    None => self.alloc_edge(a, b),
                };
            }
            let created = self.create_face_raw(&verts, &edges, Some(face.0));
            let first = self.faces.records[created as usize].first;
            if self.loops.records[first as usize].radial_next != first {
                let previous_vert =
                    self.loops.records[self.loops.records[first as usize].prev as usize].vert;
                let mut other = self.loops.records[first as usize].radial_next;
                while other != first {
                    let other_face = self.loops.records[other as usize].face;
                    let other_prev = self.loops.records[other as usize].prev;
                    if self.faces.records[other_face as usize].len == 3
                        && self.loops.records[other_prev as usize].vert == previous_vert
                    {
                        result.doubles.push(FaceId(match index != last {
                            true => created,
                            false => face.0,
                        }));
                        break;
                    }
                    other = self.loops.records[other as usize].radial_next;
                }
            }
            let created_loops = self.face_loops_vec(FaceId(created));
            for (source, target) in triangle.iter().zip(&created_loops) {
                self.copy_loop_attrs(source.0, target.0);
            }
            for &lp in &created_loops {
                if self.loop_radial_next(lp) == lp {
                    result.edges.push(self.loop_edge(lp));
                }
            }
            match index != last {
                true => result.faces.push(FaceId(created)),
                false => last_face = created,
            }
        }
        self.swap_faces(face.0, last_face);
        self.kill_face_raw(last_face);
        self.touch_topology();
        Ok(result)
    }

    fn swap_faces(&mut self, a: u32, b: u32) {
        for lp in self.face_loops_vec(FaceId(a)) {
            self.loops.records[lp.index()].face = b;
        }
        for lp in self.face_loops_vec(FaceId(b)) {
            self.loops.records[lp.index()].face = a;
        }
        self.faces.records.swap(a as usize, b as usize);
        self.face_normals.swap(a as usize, b as usize);
        let rows = self.face_data.extract(&[a, b]);
        self.face_data.copy_from(&rows, 0, b as usize);
        self.face_data.copy_from(&rows, 1, a as usize);
    }

    pub fn duplicate(&mut self, geometry: &Geometry) -> Duplicate {
        let mut verts: IdSet<VertId> = geometry
            .verts
            .iter()
            .filter(|&vert| self.contains_vert(vert))
            .collect();
        let mut edges: IdSet<EdgeId> = geometry
            .edges
            .iter()
            .filter(|&edge| self.contains_edge(edge))
            .collect();
        let faces: Vec<FaceId> = geometry
            .faces
            .iter()
            .filter(|&face| self.contains_face(face))
            .collect();
        for &face in &faces {
            for lp in self.face_loops(face) {
                edges.insert(self.loop_edge(lp));
            }
        }
        for edge in edges.iter() {
            verts.extend(self.edge_verts(edge));
        }
        let mut result = Duplicate::default();
        for vert in verts.iter() {
            let created = self.alloc_vert(self.positions[vert.index()]);
            self.copy_vert_attrs(vert.0, created);
            self.vert_data.key_index[created as usize] = KEY_INDEX_NONE;
            result.verts.insert(vert, VertId(created));
        }
        for edge in edges.iter() {
            let [a, b] = self
                .edge_verts(edge)
                .map(|vert| result.verts.copied(vert).unwrap_or(vert));
            let created = self.alloc_edge(a.0, b.0);
            self.copy_edge_attrs(edge.0, created);
            result.edges.insert(edge, EdgeId(created));
        }
        for face in faces {
            let loops = self.face_loops_vec(face);
            let face_verts: Ring<u32> = loops
                .iter()
                .map(|&lp| {
                    result
                        .verts
                        .copied(self.loop_vert(lp))
                        .map_or(NONE, |vert| vert.0)
                })
                .collect();
            let face_edges: Ring<u32> = loops
                .iter()
                .map(|&lp| {
                    result
                        .edges
                        .copied(self.loop_edge(lp))
                        .map_or(NONE, |edge| edge.0)
                })
                .collect();
            let created = self.create_face_raw(&face_verts, &face_edges, Some(face.0));
            let created_loops = self.face_loops_vec(FaceId(created));
            for (source, target) in loops.iter().zip(&created_loops) {
                self.copy_loop_attrs(source.0, target.0);
            }
            result.faces.insert(face, FaceId(created));
        }
        self.touch_topology();
        result
    }

    pub fn translate(&mut self, verts: impl IntoIterator<Item = VertId>, offset: Vec3) {
        for vert in verts {
            self.positions[vert.index()] += offset;
            self.log_moved(vert);
        }
        self.touch_moved();
    }
}

fn quad_diagonal_02(corners: &[Vec3], quad: QuadMethod) -> bool {
    let [c0, c1, c2, c3] = [corners[0], corners[1], corners[2], corners[3]];
    match quad {
        QuadMethod::Fixed => true,
        QuadMethod::Alternate => false,
        QuadMethod::ShortEdge => c1.distance_squared(c3) - c0.distance_squared(c2) > 0.0,
        QuadMethod::LongEdge => c1.distance_squared(c3) - c0.distance_squared(c2) < 0.0,
        QuadMethod::Beauty => match quad_flip_flags(c1, c2, c3, c0) {
            (true, _) => true,
            (false, true) => false,
            (false, false) => rotate_beauty_area(c1, c2, c3, c0, false) > 0.0,
        },
    }
}
