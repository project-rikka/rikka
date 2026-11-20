use rikka_mesh_edit::{EdgeId, FaceId, IdSet, VertId};
use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::faces::NgonSpec;
use crate::model::{FaceKind, HalfId};
use crate::state::Bevel;

impl Bevel<'_> {
    fn count_ccw_between(&self, from: HalfId, to: HalfId) -> usize {
        let mut count = 0;
        let mut half = from;
        loop {
            if half == to {
                break;
            }
            half = self.next_half(half);
            count += 1;
            if half == from {
                break;
            }
        }
        count
    }

    fn rebuild_polygon(&mut self, face: FaceId) -> Result<bool> {
        let mut rebuild = false;
        let mut verts: Vec<VertId> = Vec::new();
        let mut edges: Vec<EdgeId> = Vec::new();
        let mut origins: FxHashMap<VertId, VertId> = FxHashMap::default();
        for lp in self.mesh.face_loops_vec(face) {
            let vert = self.mesh.loop_vert(lp);
            if !self.mesh.is_vert_tagged(vert) {
                verts.push(vert);
                edges.push(self.mesh.loop_edge(lp));
                origins.entry(vert).or_insert(vert);
                continue;
            }
            let Some(bev) = self.bev_of(vert) else {
                continue;
            };
            let prev_loop = self.mesh.loop_prev(lp);
            let (Some(e), Some(eprev)) = (
                self.find_half(bev, self.mesh.loop_edge(lp)),
                self.find_half(bev, self.mesh.loop_edge(prev_loop)),
            ) else {
                continue;
            };
            let bme = self.half(e).edge;
            let go_ccw = if self.prev_half(e) == eprev {
                if self.prev_half(eprev) == e {
                    self.half(e).fnext != Some(face)
                } else {
                    true
                }
            } else if self.prev_half(eprev) == e {
                false
            } else {
                self.count_ccw_between(eprev, e) < self.count_ccw_between(e, eprev)
            };
            let seg = self.vmesh(bev).seg;
            let mut on_profile_start = false;
            let (mut vstart, vend) = match go_ccw {
                true => (self.half(eprev).rightv, self.half(e).leftv),
                false => (self.half(eprev).leftv, self.half(e).rightv),
            };
            let (Some(start), Some(vend)) = (vstart, vend) else {
                continue;
            };
            if go_ccw {
                if self.half(e).profile_index > 0 {
                    vstart = Some(self.bound(start).prev);
                    on_profile_start = true;
                }
            } else if self.half(eprev).profile_index > 0 {
                vstart = Some(self.bound(start).next);
                on_profile_start = true;
            }
            let mut v = vstart.expect("boundary start exists");
            if !on_profile_start && let Some(bound_vert) = self.bound(v).vert {
                verts.push(bound_vert);
                edges.push(bme);
                origins.entry(bound_vert).or_insert(vert);
            }
            let mut guard = self.vmesh(bev).count + 1;
            while v != vend && guard > 0 {
                guard -= 1;
                if go_ccw {
                    let i = self.bound(v).index;
                    let kstart = match on_profile_start {
                        true => {
                            on_profile_start = false;
                            self.half(e).profile_index
                        }
                        false => 1,
                    };
                    let kend = match self.half(eprev).rightv == Some(v)
                        && self.half(eprev).profile_index > 0
                    {
                        true => self.half(eprev).profile_index,
                        false => seg,
                    };
                    for k in kstart..=kend {
                        if let Some(grid_vert) = self.vmesh(bev).vert(i, 0, k) {
                            verts.push(grid_vert);
                            edges.push(bme);
                            origins.entry(grid_vert).or_insert(vert);
                        }
                    }
                    v = self.bound(v).next;
                } else {
                    let i = self.bound(self.bound(v).prev).index;
                    let kstart = match on_profile_start {
                        true => {
                            on_profile_start = false;
                            self.half(eprev).profile_index as isize
                        }
                        false => seg as isize - 1,
                    };
                    let kend = match self.half(e).rightv == Some(self.bound(v).prev)
                        && self.half(e).profile_index > 0
                    {
                        true => self.half(e).profile_index as isize,
                        false => 0,
                    };
                    let mut k = kstart;
                    while k >= kend {
                        if let Some(grid_vert) = self.vmesh(bev).vert(i, 0, k as usize) {
                            verts.push(grid_vert);
                            edges.push(bme);
                            origins.entry(grid_vert).or_insert(vert);
                        }
                        k -= 1;
                    }
                    v = self.bound(v).prev;
                }
            }
            rebuild = true;
        }
        if !rebuild {
            return Ok(false);
        }
        let new_face = self.create_ngon(&NgonSpec {
            facerep: Some(face),
            origins: Some(&origins),
            ..NgonSpec::new(&verts)
        })?;
        let n = verts.len();
        let mut edge_prev = edges[n - 1];
        for k in 0..n {
            let Some(created) = self.mesh.edge_between(verts[k], verts[(k + 1) % n]) else {
                continue;
            };
            if edges[k] == created {
                continue;
            }
            self.mesh.copy_edge_data(edges[k], created);
            if k < n - 1 && edges[k] == edges[k + 1] {
                if self.mesh.is_edge_seam(edges[k]) && !self.mesh.is_edge_seam(edge_prev) {
                    self.mesh.set_edge_seam(created, false);
                }
                if self.mesh.is_edge_sharp(edges[k]) && !self.mesh.is_edge_sharp(edge_prev) {
                    self.mesh.set_edge_sharp(created, false);
                }
            } else {
                edge_prev = edges[k];
            }
        }
        if let Some(new_face) = new_face {
            self.record_face_kind(new_face, FaceKind::Recon);
            self.mesh.set_face_tag(new_face, false);
            for edge in self.mesh.face_edges_vec(new_face) {
                let keep = self
                    .mesh
                    .edge_faces(edge)
                    .any(|other| self.mesh.is_face_tagged(other));
                if !keep {
                    self.out_edges.remove(edge);
                }
            }
        }
        Ok(true)
    }

    pub(crate) fn rebuild_existing_polygons(
        &mut self,
        vert: VertId,
        rebuilt: &mut IdSet<FaceId>,
        order: &mut Vec<FaceId>,
    ) -> Result<()> {
        let faces: Vec<FaceId> = self.mesh.vert_faces(vert).collect();
        for face in faces {
            if rebuilt.contains(face) {
                continue;
            }
            if self.rebuild_polygon(face)? {
                rebuilt.insert(face);
                order.push(face);
            }
        }
        Ok(())
    }

    pub(crate) fn reattach_wires(&mut self, vert: VertId) -> Result<()> {
        let Some(bev) = self.bev_of(vert) else {
            return Ok(());
        };
        let wires = self.bev(bev).wire_edges.clone();
        if wires.is_empty() || self.vmesh(bev).boundstart.is_none() {
            return Ok(());
        }
        for wire in wires {
            let other = self.other_vert(wire, vert);
            let other_bev = match self.mesh.is_vert_tagged(other) {
                true => match self.bev_of(other) {
                    Some(other_bev) if self.vmesh(other_bev).boundstart.is_some() => {
                        Some(other_bev)
                    }
                    _ => return Ok(()),
                },
                false => None,
            };
            let mut closest: Option<(VertId, VertId)> = None;
            let mut best = f32::MAX;
            for bound in self.bev_ring(bev) {
                let (co, bound_vert) = (self.bound(bound).co, self.bound(bound).vert);
                match other_bev {
                    Some(other_bev) => {
                        for other_bound in self.bev_ring(other_bev) {
                            let distance = self.bound(other_bound).co.distance_squared(co);
                            if distance < best
                                && let (Some(a), Some(b)) =
                                    (bound_vert, self.bound(other_bound).vert)
                            {
                                closest = Some((a, b));
                                best = distance;
                            }
                        }
                    }
                    None => {
                        let distance = self.position(other).distance_squared(co);
                        if distance < best
                            && let Some(a) = bound_vert
                        {
                            closest = Some((a, other));
                            best = distance;
                        }
                    }
                }
            }
            if let Some((a, b)) = closest
                && a != b
                && self.mesh.edge_between(a, b).is_none()
            {
                self.mesh.add_edge_like(a, b, wire)?;
            }
        }
        Ok(())
    }
}
