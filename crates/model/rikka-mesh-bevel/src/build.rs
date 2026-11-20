use glam::Vec3;
use rikka_mesh_edit::{EdgeId, FaceId, VertId};

use crate::error::Result;
use crate::faces::NgonSpec;
use crate::math::{EPSILON, midpoint};
use crate::model::{BevId, BoundId, FaceKind, Grid, MeshKind};
use crate::options::Affect;
use crate::spacing::{PRO_LINE_R, PRO_SQUARE_IN_R, PRO_SQUARE_R};
use crate::state::Bevel;

impl Bevel<'_> {
    pub(crate) fn create_grid_vert(&mut self, bev: BevId, at: [usize; 3]) -> Result<VertId> {
        let example = self.bev(bev).vert;
        let co = self.vmesh(bev).co(at[0], at[1], at[2]);
        let vert = self.mesh.add_vert_like(co, example)?;
        self.mesh.set_vert_tag(vert, false);
        self.out_verts.insert(vert);
        self.vmesh_mut(bev).at_mut(at[0], at[1], at[2]).vert = Some(vert);
        Ok(vert)
    }

    pub(crate) fn build_vmesh(&mut self, bev: BevId) -> Result<()> {
        let n = self.vmesh(bev).count;
        let ns = self.vmesh(bev).seg;
        self.vmesh_mut(bev).allocate();
        let weld = self.bev(bev).selcount == 2 && n == 2;
        let mut weld1: Option<BoundId> = None;
        let mut weld2: Option<BoundId> = None;
        let bev_co = self.position(self.bev(bev).vert);
        for bound in self.bev_ring(bev) {
            let i = self.bound(bound).index;
            let co = self.bound(bound).co;
            self.vmesh_mut(bev).set_co(i, 0, 0, co);
            let vert = self.create_grid_vert(bev, [i, 0, 0])?;
            self.bound_mut(bound).vert = Some(vert);
            if weld && self.bound(bound).ebev.is_some() {
                match weld1 {
                    None => weld1 = Some(bound),
                    Some(first) => {
                        weld2 = Some(bound);
                        self.set_profile_params(bev, first);
                        self.set_profile_params(bev, bound);
                        self.move_weld_profile_planes(bev_co, first, bound);
                    }
                }
            }
        }
        self.calculate_vm_profiles(bev);
        for bound in self.bev_ring(bev) {
            let i = self.bound(bound).index;
            let next = self.bound(self.bound(bound).next).index;
            self.vmesh_mut(bev).copy([i, 0, ns], [next, 0, 0]);
            if self.vmesh(bev).kind == MeshKind::Adj {
                continue;
            }
            for k in 1..ns {
                if self.bound(bound).ebev.is_some() {
                    let co = self.profile_point(&self.bound(bound).profile, k, ns);
                    self.vmesh_mut(bev).set_co(i, 0, k, co);
                    if !weld {
                        self.create_grid_vert(bev, [i, 0, k])?;
                    }
                } else if n == 2 {
                    self.vmesh_mut(bev).copy([i, 0, k], [1 - i, 0, ns - k]);
                }
            }
        }
        if weld && let (Some(weld1), Some(weld2)) = (weld1, weld2) {
            self.vmesh_mut(bev).kind = MeshKind::None;
            let (i1, i2) = (self.bound(weld1).index, self.bound(weld2).index);
            for k in 1..ns {
                let a = self.vmesh(bev).co(i1, 0, k);
                let b = self.vmesh(bev).co(i2, 0, ns - k);
                let co = if self.params.is_custom() {
                    midpoint(a, b)
                } else {
                    let (r1, r2) = (
                        self.bound(weld1).profile.super_r,
                        self.bound(weld2).profile.super_r,
                    );
                    if r1 == PRO_LINE_R && r2 != PRO_LINE_R {
                        b
                    } else if r2 == PRO_LINE_R && r1 != PRO_LINE_R {
                        a
                    } else {
                        midpoint(a, b)
                    }
                };
                self.vmesh_mut(bev).set_co(i1, 0, k, co);
                self.create_grid_vert(bev, [i1, 0, k])?;
            }
            for k in 1..ns {
                self.vmesh_mut(bev).copy([i2, 0, ns - k], [i1, 0, k]);
            }
        }
        let mut vpipe = None;
        if (3..=4).contains(&n) && self.params.seg > 1 {
            vpipe = self.pipe_test(bev);
            if vpipe.is_some() {
                self.vmesh_mut(bev).kind = MeshKind::Adj;
            }
        }
        match self.vmesh(bev).kind {
            MeshKind::None => {
                if n == 2 && self.params.affect == Affect::Vertices {
                    self.bevel_vert_two_edges(bev)?;
                }
            }
            MeshKind::Poly => {
                self.build_poly(bev)?;
            }
            MeshKind::Adj => self.build_rings(bev, vpipe)?,
            MeshKind::TriFan => self.build_trifan(bev)?,
            MeshKind::Cutoff => self.build_cutoff(bev)?,
        }
        Ok(())
    }

    fn bevel_vert_two_edges(&mut self, bev: BevId) -> Result<()> {
        let ns = self.vmesh(bev).seg;
        let vert = self.bev(bev).vert;
        if ns > 1 {
            let start = self
                .vmesh(bev)
                .boundstart
                .expect("vertex bevel has a boundary");
            let v1 = self
                .vmesh(bev)
                .vert(0, 0, 0)
                .expect("boundary vertex exists");
            let v2 = self
                .vmesh(bev)
                .vert(1, 0, 0)
                .expect("boundary vertex exists");
            let (p1, p2, middle) = (self.position(v1), self.position(v2), self.position(vert));
            let super_r = self.params.pro_super_r;
            {
                let profile = &mut self.bound_mut(start).profile;
                profile.super_r = super_r;
                profile.start = p1;
                profile.end = p2;
                profile.middle = middle;
                profile.plane_co = Vec3::ZERO;
                profile.plane_no = Vec3::ZERO;
                profile.proj_dir = Vec3::ZERO;
            }
            for k in 1..ns {
                let co = self.profile_point(&self.bound(start).profile, k, ns);
                self.vmesh_mut(bev).set_co(0, 0, k, co);
                self.create_grid_vert(bev, [0, 0, k])?;
            }
            self.vmesh_mut(bev).set_co(0, 0, ns, p2);
            for k in 1..ns {
                self.vmesh_mut(bev).copy([1, 0, ns - k], [0, 0, k]);
            }
        }
        if self.mesh.vert_faces(vert).next().is_none() {
            let example = self.half(self.bev(bev).half(0)).edge;
            for k in 0..ns {
                let (Some(a), Some(b)) = (
                    self.vmesh(bev).vert(0, 0, k),
                    self.vmesh(bev).vert(0, 0, k + 1),
                ) else {
                    continue;
                };
                let edge = match self.mesh.edge_between(a, b) {
                    Some(edge) => edge,
                    None => self.mesh.add_edge_like(a, b, example)?,
                };
                self.out_edges.insert(edge);
            }
        }
        Ok(())
    }

    fn center_ngon_corners(
        &self,
        bev: BevId,
    ) -> (Option<FaceId>, Option<EdgeId>, Option<EdgeId>, Vec<BoundId>) {
        if !self.bev(bev).any_seam {
            return (None, None, None, Vec::new());
        }
        let frep = self.frep_for_center_poly(bev);
        let vert = self.bev(bev).vert;
        let (e1, e2) = self.incident_edges(frep, vert);
        let unsnapped = self.face_internal_bounds(bev, frep);
        (frep, e1, e2, unsnapped)
    }

    fn build_center_ngon(&mut self, bev: BevId, material: Option<i32>) -> Result<()> {
        let ns2 = self.vmesh(bev).seg / 2;
        let (frep, e1, e2, unsnapped) = self.center_ngon_corners(bev);
        let mut verts = Vec::new();
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        for bound in self.bev_ring(bev) {
            let i = self.bound(bound).index;
            let vert = self
                .vmesh(bev)
                .vert(i, ns2, ns2)
                .expect("center vertex exists");
            verts.push(vert);
            match frep {
                Some(frep) => {
                    faces.push(Some(frep));
                    edges.push(match unsnapped.contains(&bound) {
                        true => None,
                        false => self.closer_edge(self.position(vert), e1, e2),
                    });
                }
                None => {
                    faces.push(self.boundvert_rep_face(bound).0);
                    edges.push(None);
                }
            }
        }
        let face = self.create_ngon(&NgonSpec {
            face_reps: Some(&faces),
            facerep: frep,
            snap_edges: Some(&edges),
            bev_vert: Some(self.bev(bev).vert),
            material,
            ..NgonSpec::new(&verts)
        })?;
        self.record(face, FaceKind::Vert);
        Ok(())
    }

    fn build_square_in_vmesh(&mut self, bev: BevId, vm1: &Grid) -> Result<()> {
        let n = self.vmesh(bev).count;
        let ns = self.vmesh(bev).seg;
        let ns2 = ns / 2;
        let odd = ns % 2 == 1;
        for i in 0..n {
            for k in 1..ns {
                self.vmesh_mut(bev).set_co(i, 0, k, vm1.co(i, 0, k));
                if i > 0 && k <= ns2 {
                    let vert = self.vmesh(bev).vert(i - 1, 0, ns - k);
                    self.vmesh_mut(bev).at_mut(i, 0, k).vert = vert;
                } else if i == n - 1 && k > ns2 {
                    let vert = self.vmesh(bev).vert(0, 0, ns - k);
                    self.vmesh_mut(bev).at_mut(i, 0, k).vert = vert;
                } else {
                    self.create_grid_vert(bev, [i, 0, k])?;
                }
            }
        }
        if odd {
            for i in 0..n {
                let vert = self.vmesh(bev).vert(i, 0, ns2);
                self.vmesh_mut(bev).at_mut(i, ns2, ns2).vert = vert;
            }
            self.build_center_ngon(bev, self.params.material)?;
        }
        Ok(())
    }

    fn build_rings(&mut self, bev: BevId, vpipe: Option<BoundId>) -> Result<()> {
        let material = self.params.material;
        let n_bndv = self.vmesh(bev).count;
        let ns = self.vmesh(bev).seg;
        let ns2 = ns / 2;
        let odd = ns % 2 == 1;
        let vm1 = if self.params.pro_super_r == PRO_SQUARE_R
            && self.bev(bev).selcount >= 3
            && !odd
            && !self.params.is_custom()
        {
            self.square_out_adj_vmesh(bev)
        } else if let Some(vpipe) = vpipe {
            self.pipe_adj_vmesh(bev, vpipe)
        } else if self.tri_corner_test(bev) == 1 {
            let vm1 = self.tri_corner_adj_vmesh(bev);
            if self.params.pro_super_r == PRO_SQUARE_IN_R && !self.params.is_custom() {
                return self.build_square_in_vmesh(bev, &vm1);
            }
            vm1
        } else {
            self.adj_vmesh(bev)
        };
        for i in 0..n_bndv {
            for j in 0..=ns2 {
                for k in 0..=ns {
                    if j == 0 && (k == 0 || k == ns) {
                        continue;
                    }
                    if !self.vmesh(bev).is_canon(i, j, k) {
                        continue;
                    }
                    self.vmesh_mut(bev).set_co(i, j, k, vm1.co(i, j, k));
                    self.create_grid_vert(bev, [i, j, k])?;
                }
            }
        }
        self.vmesh_mut(bev).copy_equivalent();
        let ring = self.bev_ring(bev);
        let mut rep_faces = vec![None; n_bndv];
        for &bound in &ring {
            rep_faces[self.bound(bound).index] = self.boundvert_rep_face(bound).0;
        }
        let edge_bevel = self.params.affect == Affect::Edges;
        let mut center_verts = vec![None; n_bndv];
        let mut center_snaps: Vec<Option<EdgeId>> = vec![None; n_bndv];
        let mut center_faces: Vec<Option<FaceId>> = vec![None; n_bndv];
        let mut frep_beats_next = vec![false; n_bndv];
        let mut center_frep = None;
        if odd && edge_bevel {
            center_frep = self.frep_for_center_poly(bev);
            for i in 0..n_bndv {
                let next = (i + 1) % n_bndv;
                let winner = self.choose_rep_face(&[rep_faces[i], rep_faces[next]]);
                frep_beats_next[i] = winner == rep_faces[i];
            }
        }
        let any_seam = self.bev(bev).any_seam;
        for &bound in &ring {
            let i = self.bound(bound).index;
            let inext = self.bound(self.bound(bound).next).index;
            let f = rep_faces[i];
            let f2 = rep_faces[inext];
            let fc = match frep_beats_next[i] {
                true => f,
                false => f2,
            };
            let (prev, next) = (self.bound(bound).prev, self.bound(bound).next);
            let (e, eprev, enext) = match edge_bevel {
                false => (
                    self.bound(bound).efirst,
                    self.bound(prev).efirst,
                    self.bound(next).efirst,
                ),
                true => (
                    self.bound(bound).ebev,
                    self.bound(prev).ebev,
                    self.bound(next).ebev,
                ),
            };
            let bme = e.map(|half| self.half(half).edge);
            let bmeprev = eprev.map(|half| self.half(half).edge);
            let bmenext = enext.map(|half| self.half(half).edge);
            for j in 0..ns2 {
                for k in 0..ns2 + usize::from(odd) {
                    let grid = self.vmesh(bev);
                    let corners = [
                        grid.vert(i, j, k),
                        grid.vert(i, j, k + 1),
                        grid.vert(i, j + 1, k + 1),
                        grid.vert(i, j + 1, k),
                    ];
                    let Some(verts) = corners.into_iter().collect::<Option<Vec<_>>>() else {
                        continue;
                    };
                    let mut fr = [f; 4];
                    let mut se: [Option<EdgeId>; 4] = [None; 4];
                    if !edge_bevel {
                        fr = [f2; 4];
                        if j < k {
                            if k == ns2 && j == ns2 - 1 {
                                se[2] = self.bound(next).efirst.map(|half| self.half(half).edge);
                                se[3] = bme;
                            }
                        } else if j == k {
                            se[0] = bme;
                            se[2] = bme;
                            if e.is_some_and(|half| !self.half(half).is_seam) {
                                fr[3] = f;
                            }
                        }
                    } else if odd {
                        let seam_edge =
                            |half: Option<crate::model::HalfId>, edge: Option<EdgeId>| match half
                                .is_some_and(|half| self.half(half).is_seam)
                            {
                                true => edge,
                                false => None,
                            };
                        let b1 = seam_edge(eprev, bmeprev);
                        let b2 = seam_edge(e, bme);
                        let b3 = seam_edge(enext, bmenext);
                        se = snap_edges_for_vmesh_vert(
                            SnapContext {
                                i,
                                j,
                                k,
                                ns,
                                ns2,
                                n_bndv,
                                rep_faces: &rep_faces,
                                center_frep,
                                frep_beats_next: &frep_beats_next,
                            },
                            [b1, b2, b3],
                        );
                        if k == ns2 {
                            if e.is_none_or(|half| self.half(half).is_seam) {
                                fr = [fc; 4];
                            } else {
                                fr = [f, f2, f2, f];
                            }
                            if j == ns2 - 1 {
                                center_verts[i] = Some(verts[3]);
                                center_snaps[i] = se[3];
                                center_faces[i] = match any_seam {
                                    true => center_frep,
                                    false => f,
                                };
                            }
                        }
                    } else {
                        if k == ns2 - 1 {
                            se[1] = bme;
                        }
                        if j == ns2 - 1 && self.bound(prev).ebev.is_some() {
                            se[3] = bmeprev;
                        }
                        se[2] = se[1].or(se[3]);
                    }
                    let face = self.create_ngon(&NgonSpec {
                        face_reps: Some(&fr),
                        snap_edges: Some(&se),
                        bev_vert: Some(self.bev(bev).vert),
                        material,
                        ..NgonSpec::new(&verts)
                    })?;
                    self.record(face, FaceKind::Vert);
                }
            }
        }
        if odd {
            if edge_bevel {
                let frep = match any_seam {
                    true => self.frep_for_center_poly(bev),
                    false => None,
                };
                if let Some(verts) = center_verts.iter().copied().collect::<Option<Vec<_>>>() {
                    let face = self.create_ngon(&NgonSpec {
                        face_reps: Some(&center_faces),
                        facerep: frep,
                        snap_edges: Some(&center_snaps),
                        bev_vert: Some(self.bev(bev).vert),
                        material,
                        ..NgonSpec::new(&verts)
                    })?;
                    self.record(face, FaceKind::Vert);
                }
            } else {
                self.build_center_ngon(bev, material)?;
            }
        }
        Ok(())
    }

    fn build_cutoff(&mut self, bev: BevId) -> Result<()> {
        let n_bndv = self.vmesh(bev).count;
        let bev_normal = self.mesh.vert_normal(self.bev(bev).vert);
        let ring = self.bev_ring(bev);
        let sqrt2 = 2.0f32.sqrt();
        for &bound in &ring {
            let i = self.bound(bound).index;
            let prev = self.bound(bound).prev;
            let mut down = self
                .bound(bound)
                .profile
                .plane_no
                .cross(self.bound(prev).profile.plane_no);
            if down.dot(bev_normal) > 0.0 {
                down = -down;
            }
            let length = (self.bound(bound).profile.height / sqrt2
                + self.bound(prev).profile.height / sqrt2)
                / 2.0;
            let corner = self.bound(bound).co + down * length;
            let prev_index = self.bound(prev).index;
            self.vmesh_mut(bev).set_co(i, 1, 0, corner);
            self.vmesh_mut(bev).set_co(prev_index, 1, 1, corner);
        }
        let mut build_center = true;
        if n_bndv == 3 {
            let grid = self.vmesh(bev);
            let (c0, c1, c2) = (grid.co(0, 1, 0), grid.co(1, 1, 0), grid.co(2, 1, 0));
            build_center &= c0.distance_squared(c1) > EPSILON;
            build_center &= c0.distance_squared(c2) > EPSILON;
            build_center &= c1.distance_squared(c2) > EPSILON;
        }
        if build_center {
            for &bound in &ring {
                let i = self.bound(bound).index;
                let vert = self.create_grid_vert(bev, [i, 1, 0])?;
                let prev_index = self.bound(self.bound(bound).prev).index;
                self.vmesh_mut(bev).at_mut(prev_index, 1, 1).vert = Some(vert);
            }
        } else {
            let vert = self.create_grid_vert(bev, [0, 1, 0])?;
            for i in 1..n_bndv {
                self.vmesh_mut(bev).at_mut(i, 1, 0).vert = Some(vert);
            }
        }
        let seg = self.params.seg;
        let material = self.params.material;
        for &bound in &ring {
            let i = self.bound(bound).index;
            let grid = self.vmesh(bev);
            let mut corners = vec![grid.vert(i, 1, 0)];
            for k in 0..=seg {
                corners.push(grid.vert(i, 0, k));
            }
            if build_center {
                corners.push(grid.vert(i, 1, 1));
            }
            let Some(verts) = corners.into_iter().collect::<Option<Vec<_>>>() else {
                continue;
            };
            self.create_ngon(&NgonSpec {
                bev_vert: Some(self.bev(bev).vert),
                material,
                ..NgonSpec::new(&verts)
            })?;
        }
        if build_center {
            let grid = self.vmesh(bev);
            let corners: Option<Vec<VertId>> = (0..n_bndv).map(|i| grid.vert(i, 1, 0)).collect();
            if let Some(verts) = corners {
                self.create_ngon(&NgonSpec {
                    bev_vert: Some(self.bev(bev).vert),
                    material,
                    ..NgonSpec::new(&verts)
                })?;
            }
        }
        Ok(())
    }

    fn build_poly(&mut self, bev: BevId) -> Result<Option<FaceId>> {
        let (repface, e1, e2, unsnapped) = self.center_ngon_corners(bev);
        let mut verts = Vec::new();
        let mut faces = Vec::new();
        let mut edges = Vec::new();
        for bound in self.bev_ring(bev) {
            let vert = self.bound(bound).vert.expect("boundary vertex exists");
            verts.push(vert);
            match repface {
                Some(repface) => {
                    faces.push(Some(repface));
                    edges.push(match unsnapped.contains(&bound) {
                        true => None,
                        false => self.closer_edge(self.position(vert), e1, e2),
                    });
                }
                None => {
                    faces.push(self.boundvert_rep_face(bound).0);
                    edges.push(None);
                }
            }
            let Some(ebev) = self.bound(bound).ebev else {
                continue;
            };
            let seg = self.half(ebev).seg;
            let index = self.bound(bound).index;
            for k in 1..seg {
                let Some(vert) = self.vmesh(bev).vert(index, 0, k) else {
                    continue;
                };
                verts.push(vert);
                match repface {
                    Some(repface) => {
                        faces.push(Some(repface));
                        let closer = self.closer_edge(self.position(vert), e1, e2);
                        edges.push(match k < seg / 2 {
                            true => None,
                            false => closer,
                        });
                    }
                    None => {
                        faces.push(self.boundvert_rep_face(bound).0);
                        edges.push(None);
                    }
                }
            }
        }
        if verts.len() <= 2 {
            return Ok(None);
        }
        let face = self.create_ngon(&NgonSpec {
            face_reps: Some(&faces),
            facerep: repface,
            snap_edges: Some(&edges),
            bev_vert: Some(self.bev(bev).vert),
            material: self.params.material,
            ..NgonSpec::new(&verts)
        })?;
        self.record(face, FaceKind::Vert);
        Ok(face)
    }

    fn build_trifan(&mut self, bev: BevId) -> Result<()> {
        let Some(mut face) = self.build_poly(bev)? else {
            return Ok(());
        };
        let first = self.mesh.face_first_loop(face);
        let mut l_fan = self.mesh.loop_prev(first);
        let v_fan = self.mesh.loop_vert(l_fan);
        while self.mesh.face_len(face) > 3 {
            let target = self.mesh.loop_next(self.mesh.loop_next(l_fan));
            let split = self.mesh.split_face(face, l_fan, target, None, false)?;
            let new_face = split.face;
            let l_new = split.lp;
            self.out_edges.insert(self.mesh.loop_edge(l_new));
            if self.mesh.face_len(new_face) > self.mesh.face_len(face) {
                face = new_face;
                if self.mesh.loop_vert(l_new) == v_fan {
                    l_fan = l_new;
                } else if self.mesh.loop_vert(self.mesh.loop_next(l_new)) == v_fan {
                    l_fan = self.mesh.loop_next(l_new);
                } else if self.mesh.loop_vert(self.mesh.loop_prev(l_new)) == v_fan {
                    l_fan = self.mesh.loop_prev(l_new);
                }
            } else if self.mesh.loop_vert(l_fan) != v_fan {
                if self.mesh.loop_vert(self.mesh.loop_next(l_fan)) == v_fan {
                    l_fan = self.mesh.loop_next(l_fan);
                } else if self.mesh.loop_vert(self.mesh.loop_prev(l_fan)) == v_fan {
                    l_fan = self.mesh.loop_prev(l_fan);
                }
            }
            self.record_face_kind(new_face, FaceKind::Vert);
        }
        Ok(())
    }
}

struct SnapContext<'a> {
    i: usize,
    j: usize,
    k: usize,
    ns: usize,
    ns2: usize,
    n_bndv: usize,
    rep_faces: &'a [Option<FaceId>],
    center_frep: Option<FaceId>,
    frep_beats_next: &'a [bool],
}

fn center_vert_snap(
    context: &SnapContext,
    i: usize,
    eprev: Option<EdgeId>,
    enext: Option<EdgeId>,
) -> Option<EdgeId> {
    let n = context.n_bndv;
    let previ = (i + n - 1) % n;
    let nexti = (i + 1) % n;
    if context.frep_beats_next[previ] && context.rep_faces[previ] == context.center_frep {
        return eprev;
    }
    if !context.frep_beats_next[i] && context.rep_faces[nexti] == context.center_frep {
        return enext;
    }
    None
}

fn snap_edges_for_vmesh_vert(
    context: SnapContext,
    edges: [Option<EdgeId>; 3],
) -> [Option<EdgeId>; 4] {
    let [eprev, enext, enextnext] = edges;
    let mut snaps = [None; 4];
    if context.ns.is_multiple_of(2) {
        return snaps;
    }
    let (i, j, k, ns2) = (context.i, context.j, context.k, context.ns2);
    let previ = (i + context.n_bndv - 1) % context.n_bndv;
    for (corner, snap) in snaps.iter_mut().enumerate() {
        let jj = match corner < 2 {
            true => j,
            false => j + 1,
        };
        let kk = match corner == 0 || corner == 3 {
            true => k,
            false => k + 1,
        };
        *snap = if jj < ns2 && kk < ns2 {
            None
        } else if jj < ns2 && kk == ns2 {
            (!context.frep_beats_next[i]).then_some(enext).flatten()
        } else if jj < ns2 && kk == ns2 + 1 {
            context.frep_beats_next[i].then_some(enext).flatten()
        } else if jj == ns2 && kk < ns2 {
            context.frep_beats_next[previ].then_some(eprev).flatten()
        } else if jj == ns2 && kk == ns2 {
            center_vert_snap(&context, i, eprev, enext)
        } else if jj == ns2 && kk == ns2 + 1 {
            center_vert_snap(&context, (i + 1) % context.n_bndv, enext, enextnext)
        } else {
            None
        };
    }
    snaps
}
