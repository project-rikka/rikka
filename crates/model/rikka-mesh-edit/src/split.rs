use glam::Vec3;

use crate::data::KEY_INDEX_NONE;
use crate::error::{Error, Result};
use crate::ids::{EdgeId, FaceId, LoopId, NONE, VertId};
use crate::mesh::EditMesh;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeSplit {
    pub vert: VertId,
    pub edge: EdgeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceSplit {
    pub face: FaceId,
    pub lp: LoopId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacePathSplit {
    pub face: FaceId,
    pub lp: LoopId,
    pub verts: Vec<VertId>,
}

impl EditMesh {
    pub(crate) fn disk_vert_replace(&mut self, edge: u32, target: u32, source: u32) {
        self.disk_remove(edge, source);
        let record = &mut self.edges.records[edge as usize];
        if record.verts[0] == source {
            record.verts[0] = target;
        } else {
            record.verts[1] = target;
        }
        self.disk_append(edge, target);
    }

    pub(crate) fn edge_vert_swap(&mut self, edge: u32, target: u32, source: u32) {
        let first = self.edges.records[edge as usize].first_loop;
        if first != NONE {
            let mut lp = first;
            loop {
                let record = self.loops.records[lp as usize];
                if record.vert == source {
                    self.loops.records[lp as usize].vert = target;
                } else if self.loops.records[record.next as usize].vert == source {
                    self.loops.records[record.next as usize].vert = target;
                }
                lp = record.radial_next;
                if lp == first {
                    break;
                }
            }
        }
        self.disk_vert_replace(edge, target, source);
    }

    pub fn split_edge_make_vert(&mut self, edge: EdgeId, vert: VertId) -> Result<EdgeSplit> {
        self.check_edge(edge)?;
        self.check_vert(vert)?;
        if !self.edge_has_vert(edge, vert) {
            return Err(Error::VertNotInEdge(vert, edge));
        }
        let (tv, e) = (vert.0, edge.0);
        let new_vert = self.alloc_vert(self.positions[tv as usize]);
        self.copy_vert_attrs(tv, new_vert);
        self.vert_data.key_index[new_vert as usize] = KEY_INDEX_NONE;
        let new_edge = self.alloc_edge(tv, new_vert);
        self.copy_edge_attrs(e, new_edge);
        self.disk_remove(new_edge, tv);
        self.disk_remove(new_edge, new_vert);
        self.disk_vert_replace(e, new_vert, tv);
        self.disk_append(new_edge, new_vert);
        self.disk_append(new_edge, tv);

        let radial: Vec<u32> = self.edge_loops(edge).map(|lp| lp.0).collect();
        self.edges.records[e as usize].first_loop = NONE;
        for &lp in &radial {
            let record = &mut self.loops.records[lp as usize];
            record.radial_next = NONE;
            record.radial_prev = NONE;
            record.edge = NONE;
        }
        for lp in radial {
            let face = self.loops.records[lp as usize].face;
            self.faces.records[face as usize].len += 1;
            let created = self.alloc_loop(new_vert, NONE, face);
            self.copy_loop_attrs(lp, created);
            let next = self.loops.records[lp as usize].next;
            self.loops.records[created as usize].prev = lp;
            self.loops.records[created as usize].next = next;
            self.loops.records[lp as usize].next = created;
            self.loops.records[next as usize].prev = created;
            let next_vert = self.loops.records[next as usize].vert;
            let pair = self.edges.records[e as usize].verts;
            let (created_edge, old_edge) =
                match pair.contains(&new_vert) && pair.contains(&next_vert) {
                    true => (e, new_edge),
                    false => (new_edge, e),
                };
            self.radial_append(created_edge, created);
            self.radial_append(old_edge, lp);
        }
        self.touch_topology();
        Ok(EdgeSplit {
            vert: VertId(new_vert),
            edge: EdgeId(new_edge),
        })
    }

    pub fn split_edge(&mut self, edge: EdgeId, vert: VertId, factor: f32) -> Result<EdgeSplit> {
        let other = self
            .edge_other_vert(edge, vert)
            .ok_or(Error::VertNotInEdge(vert, edge))?;
        let split = self.split_edge_make_vert(edge, vert)?;
        let start = self.positions[vert.index()];
        let end = self.positions[other.index()];
        self.positions[split.vert.index()] = start + (end - start) * factor;
        let flags = self.edges.records[edge.index()].flags;
        self.edges.records[split.edge.index()].flags = flags;
        self.copy_edge_attrs(edge.0, split.edge.0);
        self.interpolate_face_vert_edge(other, split.vert, edge, factor);
        self.mix_pair(crate::Domain::Point, vert.0, other.0, split.vert.0, factor);
        self.touch_shape();
        Ok(split)
    }

    pub fn split_edge_n(&mut self, edge: EdgeId, cuts: usize) -> Result<Vec<VertId>> {
        self.check_edge(edge)?;
        let mut verts = vec![VertId(NONE); cuts];
        for index in 0..cuts {
            let factor = 1.0 / (cuts + 1 - index) as f32;
            let end = self.edge_verts(edge)[1];
            let split = self.split_edge(edge, end, factor)?;
            verts[cuts - index - 1] = split.vert;
        }
        Ok(verts)
    }

    pub fn split_face_make_edge(
        &mut self,
        face: FaceId,
        a: LoopId,
        b: LoopId,
        example: Option<EdgeId>,
        no_double: bool,
    ) -> Result<FaceSplit> {
        self.check_face(face)?;
        self.check_loop(a)?;
        self.check_loop(b)?;
        if self.loop_face(a) != face || self.loop_face(b) != face {
            return Err(Error::LoopsInDifferentFaces);
        }
        let (l_v1, l_v2) = (a.0, b.0);
        let v1 = self.loops.records[l_v1 as usize].vert;
        let v2 = self.loops.records[l_v2 as usize].vert;
        if v1 == v2 {
            return Err(Error::DegenerateEdge);
        }
        if let Some(example) = example {
            self.check_edge(example)?;
        }
        let edge = match (no_double, self.edge_between(VertId(v1), VertId(v2))) {
            (true, Some(existing)) => existing.0,
            _ => {
                let created = self.alloc_edge(v1, v2);
                if let Some(example) = example {
                    self.copy_edge_attrs(example.0, created);
                }
                created
            }
        };
        let f = face.0;
        let f2 = self.alloc_face();
        self.copy_face_attrs(f, f2);
        let l_f1 = self.alloc_loop(v2, edge, f);
        self.copy_loop_attrs(l_v2, l_f1);
        let l_f2 = self.alloc_loop(v1, edge, f2);
        self.copy_loop_attrs(l_v1, l_f2);

        let prev_v2 = self.loops.records[l_v2 as usize].prev;
        let prev_v1 = self.loops.records[l_v1 as usize].prev;
        self.loops.records[l_f1 as usize].prev = prev_v2;
        self.loops.records[l_f2 as usize].prev = prev_v1;
        self.loops.records[prev_v2 as usize].next = l_f1;
        self.loops.records[prev_v1 as usize].next = l_f2;
        self.loops.records[l_f1 as usize].next = l_v1;
        self.loops.records[l_f2 as usize].next = l_v2;
        self.loops.records[l_v1 as usize].prev = l_f1;
        self.loops.records[l_v2 as usize].prev = l_f2;

        let first = self.faces.records[f as usize].first;
        let first_in_f1 = self.loop_cycle(LoopId(l_f1)).any(|lp| lp.0 == first);
        let first_prev = self.loops.records[first as usize].prev;
        let first_next = self.loops.records[first as usize].next;
        if first_in_f1 {
            self.faces.records[f2 as usize].first = if first_prev == l_f1 {
                self.loops.records[l_f2 as usize].prev
            } else if first_next == l_f1 {
                self.loops.records[l_f2 as usize].next
            } else {
                l_f2
            };
        } else {
            self.faces.records[f2 as usize].first = first;
            self.faces.records[f as usize].first = if first_prev == l_f2 {
                self.loops.records[l_f1 as usize].prev
            } else if first_next == l_f2 {
                self.loops.records[l_f1 as usize].next
            } else {
                l_f1
            };
        }
        let second_loops: Vec<u32> = self
            .loop_cycle(LoopId(self.faces.records[f2 as usize].first))
            .map(|lp| lp.0)
            .collect();
        for &lp in &second_loops {
            self.loops.records[lp as usize].face = f2;
        }
        self.radial_append(edge, l_f1);
        self.radial_append(edge, l_f2);
        self.faces.records[f2 as usize].len = second_loops.len() as u32;
        let first_len = self
            .loop_cycle(LoopId(self.faces.records[f as usize].first))
            .count();
        self.faces.records[f as usize].len = first_len as u32;
        self.touch_topology();
        Ok(FaceSplit {
            face: FaceId(f2),
            lp: LoopId(l_f2),
        })
    }

    pub fn split_face(
        &mut self,
        face: FaceId,
        a: LoopId,
        b: LoopId,
        example: Option<EdgeId>,
        no_double: bool,
    ) -> Result<FaceSplit> {
        self.check_loop(a)?;
        self.check_loop(b)?;
        if self.loop_face(a) != face || self.loop_face(b) != face {
            return Err(Error::LoopsInDifferentFaces);
        }
        if self.is_loop_adjacent(a, b) {
            return Err(Error::AdjacentLoops);
        }
        self.split_face_make_edge(face, a, b, example, no_double)
    }

    pub fn split_face_n(
        &mut self,
        face: FaceId,
        a: LoopId,
        b: LoopId,
        positions: &[Vec3],
        example: Option<EdgeId>,
    ) -> Result<FacePathSplit> {
        self.check_loop(a)?;
        self.check_loop(b)?;
        if self.loop_face(a) != face || self.loop_face(b) != face {
            return Err(Error::LoopsInDifferentFaces);
        }
        if positions.is_empty() && self.is_loop_adjacent(a, b) {
            return Err(Error::AdjacentLoops);
        }
        let sample = self.sample_face(face);
        let vert_b = self.loop_vert(b);
        let split = self.split_face_make_edge(face, a, b, example, false)?;
        let mut edge = self.loop_edge(split.lp);
        let mut verts = Vec::with_capacity(positions.len());
        for &position in positions {
            let made = self.split_edge_make_vert(edge, vert_b)?;
            self.positions[made.vert.index()] = position;
            for target in [edge, made.edge] {
                let loops: Vec<LoopId> = self.edge_loops(target).collect();
                for lp in loops {
                    if self.loop_vert(lp) == made.vert {
                        self.interpolate_loop_from_sample(lp, &sample, true);
                    }
                }
            }
            verts.push(made.vert);
            edge = made.edge;
        }
        self.touch_shape();
        Ok(FacePathSplit {
            face: split.face,
            lp: split.lp,
            verts,
        })
    }

    pub fn connect_verts(&mut self, a: VertId, b: VertId) -> Result<FaceSplit> {
        self.check_vert(a)?;
        self.check_vert(b)?;
        let (lp_a, lp_b) = self
            .vert_pair_share_face_by_len(a, b, false)
            .ok_or(Error::NoSharedFace)?;
        let face = self.loop_face(lp_a);
        self.split_face(face, lp_a, lp_b, None, true)
    }
}
