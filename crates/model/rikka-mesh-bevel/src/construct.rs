use std::f32::consts::PI;

use glam::Vec3;
use rikka_mesh_edit::{EdgeId, FaceId, IdSet, VertId};

use crate::math::{EPSILON, angle_normalized, angle_vectors, normalize};
use crate::model::{BevId, BevVert, EdgeHalf, Grid};
use crate::options::{Affect, WidthType};
use crate::state::Bevel;

struct Ordering {
    edges: Vec<Option<EdgeId>>,
    chosen: IdSet<EdgeId>,
}

impl Bevel<'_> {
    pub(crate) fn construct(&mut self, vert: VertId) -> Option<BevId> {
        let vertex_only = self.params.affect == Affect::Vertices;
        let mut ordering = Ordering {
            edges: Vec::new(),
            chosen: IdSet::new(),
        };
        let mut nsel = 0;
        let mut tot_edges = 0;
        let mut wire_edges = Vec::new();
        let mut first = None;
        for edge in self.mesh.vert_edges(vert).collect::<Vec<_>>() {
            let face_count = self.mesh.edge_face_count(edge);
            if self.mesh.is_edge_tagged(edge) && !vertex_only {
                nsel += 1;
                first.get_or_insert(edge);
            }
            if face_count == 1 {
                first = Some(edge);
            }
            if face_count > 0 || vertex_only {
                tot_edges += 1;
            }
            if self.mesh.is_edge_wire(edge) {
                wire_edges.push(edge);
                if !vertex_only {
                    ordering.chosen.insert(edge);
                }
            }
        }
        let first = first.or_else(|| self.mesh.vert_first_edge(vert));
        if (nsel == 0 && !vertex_only) || (tot_edges < 2 && vertex_only) {
            self.mesh.set_vert_tag(vert, false);
            return None;
        }
        let first = first?;
        let bev = BevId(self.bev_verts.len() as u32);
        let first_half = self.halves.len() as u32;
        self.bev_verts.push(BevVert {
            vert,
            edgecount: tot_edges,
            selcount: nsel,
            offset: self.params.offset,
            any_seam: false,
            first_half,
            wire_edges,
            vmesh: Grid {
                seg: self.params.seg,
                ..Grid::default()
            },
        });
        self.vert_bev.insert(vert, bev);
        ordering.edges = vec![None; tot_edges];
        let faces = self.order_edges(vert, first, &mut ordering);
        for (local, edge) in ordering.edges.iter().enumerate() {
            let edge = edge.expect("bevel edge order is complete");
            let is_bev = self.mesh.is_edge_tagged(edge) && !vertex_only;
            self.halves.push(EdgeHalf {
                owner: bev,
                edge,
                fprev: faces[local].0,
                fnext: faces[local].1,
                leftv: None,
                rightv: None,
                profile_index: 0,
                seg: match is_bev {
                    true => self.params.seg,
                    false => 0,
                },
                offset_l: 0.0,
                offset_r: 0.0,
                offset_l_spec: 0.0,
                offset_r_spec: 0.0,
                is_bev,
                is_rev: self.mesh.edge_verts(edge)[1] == vert,
                is_seam: false,
                visited_rpo: false,
            });
        }
        self.orient_ccw(bev);
        self.assign_offsets(bev);
        Some(bev)
    }

    fn order_edges(
        &self,
        vert: VertId,
        first: EdgeId,
        ordering: &mut Ordering,
    ) -> Vec<(Option<FaceId>, Option<FaceId>)> {
        let total = ordering.edges.len();
        let mut first = Some(first);
        let mut i = 0;
        while let Some(start) = first {
            ordering.edges[i] = Some(start);
            ordering.chosen.insert(start);
            if i == 0 && self.fast_edge_order(vert, ordering) {
                break;
            }
            i = self.extend_edge_order(vert, ordering, i);
            i += 1;
            if i >= total {
                break;
            }
            first = None;
            for edge in self.mesh.vert_edges(vert) {
                if ordering.chosen.contains(edge) {
                    continue;
                }
                first.get_or_insert(edge);
                if self.mesh.edge_face_count(edge) == 1 {
                    first = Some(edge);
                    break;
                }
            }
        }
        let mut faces = vec![(None, None); total];
        for i in 0..total {
            let next = (i + 1) % total;
            if faces[i].1.is_some() || faces[next].0.is_some() {
                continue;
            }
            let (Some(edge), Some(edge2)) = (ordering.edges[i], ordering.edges[next]) else {
                continue;
            };
            let mut best = None;
            for lp in self.mesh.edge_loops(edge) {
                let face = self.mesh.loop_face(lp);
                let joins = self.mesh.loop_edge(self.mesh.loop_prev(lp)) == edge2
                    || self.mesh.loop_edge(self.mesh.loop_next(lp)) == edge2;
                if joins && (best.is_none() || self.mesh.loop_vert(lp) == vert) {
                    best = Some(face);
                }
                if best.is_some() {
                    faces[i].1 = best;
                    faces[next].0 = best;
                }
            }
        }
        faces
    }

    fn fast_edge_order(&self, vert: VertId, ordering: &mut Ordering) -> bool {
        let total = ordering.edges.len();
        let Some(mut edge) = ordering.edges[0] else {
            return false;
        };
        if self.mesh.edge_first_loop(edge).is_none() {
            return false;
        }
        for i in 1..total {
            let mut shared = 0;
            let mut first_successor = None;
            for candidate in self.mesh.vert_edges(vert) {
                if ordering.chosen.contains(candidate) {
                    continue;
                }
                for face in self.mesh.edge_faces(candidate) {
                    if self.mesh.face_edge_share_loop(face, edge).is_some() {
                        shared += 1;
                        first_successor.get_or_insert(candidate);
                    }
                }
                if shared >= 3 {
                    break;
                }
            }
            match first_successor {
                Some(successor) if shared == 1 || (i == 1 && shared == 2) => {
                    ordering.edges[i] = Some(successor);
                    ordering.chosen.insert(successor);
                    edge = successor;
                }
                _ => {
                    for k in 1..i {
                        if let Some(edge) = ordering.edges[k].take() {
                            ordering.chosen.remove(edge);
                        }
                    }
                    return false;
                }
            }
        }
        true
    }

    fn edges_face_connected_at_vert(&self, a: EdgeId, b: EdgeId) -> bool {
        self.mesh.edge_loops(a).any(|lp| {
            self.mesh.loop_edge(self.mesh.loop_prev(lp)) == b
                || self.mesh.loop_edge(self.mesh.loop_next(lp)) == b
        })
    }

    fn extend_edge_order(&self, vert: VertId, ordering: &mut Ordering, i: usize) -> usize {
        let Some(edge) = ordering.edges[i] else {
            return i;
        };
        let successors: Vec<EdgeId> = self
            .mesh
            .edge_loops(edge)
            .map(|lp| match self.mesh.loop_vert(lp) == vert {
                true => self.mesh.loop_edge(self.mesh.loop_prev(lp)),
                false => self.mesh.loop_edge(self.mesh.loop_next(lp)),
            })
            .filter(|&candidate| !ordering.chosen.contains(candidate))
            .collect();
        let mut best = i;
        let mut saved: Vec<EdgeId> = Vec::new();
        for next in successors {
            if i + 1 >= ordering.edges.len() || ordering.chosen.contains(next) {
                continue;
            }
            ordering.edges[i + 1] = Some(next);
            ordering.chosen.insert(next);
            let tried = self.extend_edge_order(vert, ordering, i + 1);
            let connected = match (ordering.edges[tried], ordering.edges[0]) {
                (Some(end), Some(start)) => self.edges_face_connected_at_vert(end, start),
                _ => false,
            };
            if tried > best || (tried == best && connected) {
                best = tried;
                saved = ordering.edges[i + 1..=best]
                    .iter()
                    .flatten()
                    .copied()
                    .collect();
            }
            for k in i + 1..=tried {
                if let Some(edge) = ordering.edges[k].take() {
                    ordering.chosen.remove(edge);
                }
            }
        }
        if best > i {
            for (k, &edge) in (i + 1..=best).zip(saved.iter()) {
                ordering.edges[k] = Some(edge);
                ordering.chosen.insert(edge);
            }
        }
        best
    }

    fn ccw_test(&self, a: EdgeId, b: EdgeId, face: Option<FaceId>) -> i32 {
        let Some(face) = face else {
            return 0;
        };
        let (Some(la), Some(lb)) = (
            self.mesh.face_edge_share_loop(face, a),
            self.mesh.face_edge_share_loop(face, b),
        ) else {
            return 0;
        };
        match self.mesh.loop_next(lb) == la {
            true => 1,
            false => -1,
        }
    }

    fn orient_ccw(&mut self, bev: BevId) {
        let count = self.bev(bev).edgecount;
        if count <= 1 {
            return;
        }
        let first = self.bev(bev).first_half as usize;
        let mut sum = 0;
        for i in 0..count {
            let half = &self.halves[first + i];
            let next = &self.halves[first + (i + 1) % count];
            sum += self.ccw_test(half.edge, next.edge, half.fnext);
        }
        if sum >= 0 {
            return;
        }
        let halves = &mut self.halves[first..first + count];
        halves.reverse();
        for half in halves.iter_mut() {
            std::mem::swap(&mut half.fprev, &mut half.fnext);
        }
    }

    fn face_angle(&self, half: &EdgeHalf) -> f32 {
        match (half.fprev, half.fnext) {
            (Some(prev), Some(next)) => {
                PI - angle_normalized(self.face_normal(prev), self.face_normal(next))
            }
            _ => 0.0,
        }
    }

    fn assign_offsets(&mut self, bev: BevId) {
        let vert = self.bev(bev).vert;
        let vertex_only = self.params.affect == Affect::Vertices;
        let mut axis = Vec3::ZERO;
        if vertex_only {
            if let Some(group) = self.params.vertex_group {
                let weight = self.mesh.vert_weight(vert, group);
                self.bev_mut(bev).offset *= weight;
            } else if let Some(weight) = self.params.vert_weight(vert) {
                self.bev_mut(bev).offset *= weight;
            }
            if matches!(self.params.offset_type, WidthType::Width | WidthType::Depth) {
                for half in self.bev(bev).halves() {
                    let other = self.mesh.edge_other_vert(self.half(half).edge, vert);
                    let other = other.expect("bevel edge touches its vertex");
                    axis += normalize(self.position(vert) - self.position(other));
                }
            }
        }
        let offset = self.params.offset;
        let bev_offset = self.bev(bev).offset;
        for half in self.bev(bev).halves() {
            let state = self.half(half).clone();
            let (left, right) = if state.is_bev {
                let (left, right) = match self.params.offset_type {
                    WidthType::Offset => (offset, offset),
                    WidthType::Width => {
                        let z = (2.0 * (self.face_angle(&state) / 2.0).sin()).abs();
                        let spec = match z < EPSILON {
                            true => 0.01 * offset,
                            false => offset / z,
                        };
                        (spec, spec)
                    }
                    WidthType::Depth => {
                        let z = (self.face_angle(&state) / 2.0).cos().abs();
                        let spec = match z < EPSILON {
                            true => 0.01 * offset,
                            false => offset / z,
                        };
                        (spec, spec)
                    }
                    WidthType::Percent => {
                        let prev = self.half(self.prev_half(half)).edge;
                        let next = self.half(self.next_half(half)).edge;
                        (
                            self.mesh.edge_length(prev) * offset / 100.0,
                            self.mesh.edge_length(next) * offset / 100.0,
                        )
                    }
                    WidthType::Absolute => (offset, offset),
                };
                match self.params.edge_weight(state.edge) {
                    Some(weight) => (left * weight, right * weight),
                    None => (left, right),
                }
            } else if vertex_only {
                let edge_dir = || {
                    let other = self.mesh.edge_other_vert(state.edge, vert);
                    let other = other.expect("bevel edge touches its vertex");
                    self.position(vert) - self.position(other)
                };
                let spec = match self.params.offset_type {
                    WidthType::Offset | WidthType::Absolute => bev_offset,
                    WidthType::Width => {
                        let z = (2.0 * angle_vectors(axis, edge_dir()).sin()).abs();
                        match z < EPSILON {
                            true => 0.01 * offset,
                            false => offset / z,
                        }
                    }
                    WidthType::Depth => {
                        let z = angle_vectors(axis, edge_dir()).cos().abs();
                        match z < EPSILON {
                            true => 0.01 * offset,
                            false => offset / z,
                        }
                    }
                    WidthType::Percent => self.mesh.edge_length(state.edge) * bev_offset / 100.0,
                };
                (spec, spec)
            } else {
                (0.0, 0.0)
            };
            let is_seam = match (state.fprev, state.fnext) {
                (Some(prev), Some(next)) => !self.contig_ldata_across_edge(state.edge, prev, next),
                _ => true,
            };
            let entry = self.half_mut(half);
            entry.offset_l_spec = left;
            entry.offset_r_spec = right;
            entry.offset_l = left;
            entry.offset_r = right;
            entry.is_seam = is_seam;
        }
    }
}
