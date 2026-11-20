use rikka_mesh_edit::{EdgeId, FaceId, VertId};
use rustc_hash::FxHashMap;

use crate::error::Result;
use crate::faces::NgonSpec;
use crate::model::{BevId, FaceKind, HalfId};
use crate::state::Bevel;

impl Bevel<'_> {
    fn is_weld_cross(&self, bev: BevId) -> bool {
        let state = self.bev(bev);
        if state.edgecount != 4 || state.selcount != 2 {
            return false;
        }
        let bevelled = |local: usize| self.half(state.half(local)).is_bev;
        (bevelled(0) && bevelled(2)) || (bevelled(1) && bevelled(3))
    }

    fn weld_cross_attrs_copy(&mut self, bev: BevId, index: usize, half: HalfId) {
        let state = self.bev(bev);
        let Some(local) = (0..4).find(|&local| state.half(local) == half) else {
            return;
        };
        let edge_prev = self.half(state.half((local + 3) % 4)).edge;
        let edge_next = self.half(state.half((local + 1) % 4)).edge;
        let disable_seam = self.mesh.is_edge_seam(edge_prev) != self.mesh.is_edge_seam(edge_next);
        let enable_smooth =
            self.mesh.is_edge_sharp(edge_prev) != self.mesh.is_edge_sharp(edge_next);
        let segments = self.half(half).seg;
        for i in 0..segments {
            let grid = self.vmesh(bev);
            let (Some(a), Some(b)) = (grid.vert(index, 0, i), grid.vert(index, 0, i + 1)) else {
                continue;
            };
            let Some(edge) = self.mesh.edge_between(a, b) else {
                continue;
            };
            self.mesh.copy_edge_data(edge_prev, edge);
            if disable_seam {
                self.mesh.set_edge_seam(edge, false);
            }
            if enable_smooth {
                self.mesh.set_edge_sharp(edge, false);
            }
        }
    }

    pub(crate) fn build_edge_polygons(&mut self, edge: EdgeId) -> Result<()> {
        if !self.mesh.is_edge_manifold(edge) {
            return Ok(());
        }
        let [v1, v2] = self.mesh.edge_verts(edge);
        let (Some(bv1), Some(bv2)) = (self.bev_of(v1), self.bev_of(v2)) else {
            return Ok(());
        };
        let (Some(e1), Some(e2)) = (self.find_half(bv1, edge), self.find_half(bv2, edge)) else {
            return Ok(());
        };
        let material = self.params.material;
        let nseg = self.half(e1).seg;
        let h1 = self.half(e1).clone();
        let h2 = self.half(e2).clone();
        let (Some(l1), Some(r1), Some(l2), Some(r2)) = (h1.leftv, h1.rightv, h2.leftv, h2.rightv)
        else {
            return Ok(());
        };
        let (Some(bmv1), Some(bmv4), Some(bmv2), Some(bmv3)) = (
            self.bound(l1).vert,
            self.bound(r1).vert,
            self.bound(r2).vert,
            self.bound(l2).vert,
        ) else {
            return Ok(());
        };
        let f1 = h1.fprev;
        let f2 = h1.fnext;
        let faces = [f1, f1, f2, f2];
        let i1 = self.bound(l1).index;
        let i2 = self.bound(l2).index;
        let (orig1, orig2) = (self.bev(bv1).vert, self.bev(bv2).vert);
        let mut verts = [bmv1, bmv2, bmv2, bmv1];
        let mut origins: FxHashMap<VertId, VertId> = FxHashMap::default();
        origins.insert(verts[0], orig1);
        origins.insert(verts[1], orig2);
        let odd = nseg % 2 == 1;
        let mid = nseg / 2;
        let mut choice = None;
        let mut center_adj_k = None;
        if odd && h1.is_seam {
            choice = self.choose_rep_face(&[f1, f2]);
            if nseg > 1 {
                center_adj_k = Some(match choice == f1 {
                    true => mid + 2,
                    false => mid,
                });
            }
        }
        for k in 1..=nseg {
            let (Some(a), Some(b)) = (
                self.vmesh(bv1).vert(i1, 0, k),
                self.vmesh(bv2).vert(i2, 0, nseg - k),
            ) else {
                continue;
            };
            verts[3] = a;
            verts[2] = b;
            origins.entry(a).or_insert(orig1);
            origins.entry(b).or_insert(orig2);
            let mut spec = NgonSpec {
                origins: Some(&origins),
                material,
                ..NgonSpec::new(&verts)
            };
            let snaps_left = [None, None, Some(edge), Some(edge)];
            let snaps_right = [Some(edge), Some(edge), None, None];
            if odd && k == mid + 1 {
                if h1.is_seam {
                    spec.facerep = choice;
                    spec.snap_edges = Some(match choice == f1 {
                        true => &snaps_left,
                        false => &snaps_right,
                    });
                } else {
                    spec.face_reps = Some(&faces);
                    spec.facerep = choice;
                }
            } else if odd && Some(k) == center_adj_k && h1.is_seam {
                if k == mid {
                    spec.snap_edges = Some(&snaps_left);
                    spec.facerep = f1;
                } else {
                    spec.snap_edges = Some(&snaps_right);
                    spec.facerep = f2;
                }
            } else if !odd && k == mid {
                spec.snap_edges = Some(&snaps_left);
                spec.facerep = f1;
            } else if !odd && k == mid + 1 {
                spec.snap_edges = Some(&snaps_right);
                spec.facerep = f2;
            } else {
                spec.facerep = match k <= mid {
                    true => f1,
                    false => f2,
                };
            }
            let face = self.create_ngon(&spec)?;
            if let Some(face) = face {
                self.record_face_kind(face, FaceKind::Edge);
                self.tag_long_loops(face, verts[0], verts[2]);
            }
            verts[0] = verts[3];
            verts[1] = verts[2];
        }
        if let Some(edge1) = self.mesh.edge_between(bmv1, bmv2) {
            self.mesh.copy_edge_data(edge, edge1);
        }
        if let Some(edge2) = self.mesh.edge_between(bmv3, bmv4) {
            self.mesh.copy_edge_data(edge, edge2);
        }
        if self.is_weld_cross(bv1) {
            self.weld_cross_attrs_copy(bv1, i1, e1);
        }
        if self.is_weld_cross(bv2) {
            self.weld_cross_attrs_copy(bv2, i2, e2);
        }
        Ok(())
    }

    fn tag_long_loops(&mut self, face: FaceId, a: VertId, b: VertId) {
        for lp in self.mesh.face_loops_vec(face) {
            let vert = self.mesh.loop_vert(lp);
            if vert == a || vert == b {
                self.long_loops.insert(lp);
            }
        }
    }
}
