use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use glam::Vec3;

use crate::math::{
    Affine, EPSILON, EPSILON_ANG, EPSILON_BIG, EPSILON_D, EPSILON_SQ, LineMeet, Plane, SMALL_ANG,
    angle_normalized, angle_three, closest_to_segment, compare, compare_vec3, lerp, line_line,
    midpoint, normalize,
};
use crate::model::{BevId, BoundId, Grid};
use crate::offset::AngleKind;
use crate::options::Affect;
use crate::profile::snap_to_superellipsoid;
use crate::spacing::{PRO_SQUARE_IN_R, PRO_SQUARE_R};
use crate::state::Bevel;

const FRAC_1_SQRT_3: f32 = 0.577_350_26;

fn closer(a: Vec3, b: Vec3, target: Vec3) -> Vec3 {
    match a.distance_squared(target) <= b.distance_squared(target) {
        true => a,
        false => b,
    }
}

impl Bevel<'_> {
    pub(crate) fn pipe_test(&self, bev: BevId) -> Option<BoundId> {
        let grid = self.vmesh(bev);
        let selcount = self.bev(bev).selcount;
        if !(3..=4).contains(&grid.count) || !(3..=4).contains(&selcount) {
            return None;
        }
        let vert = self.bev(bev).vert;
        let here = self.position(vert);
        let mut found = None;
        for v1 in self.bev_ring(bev) {
            let v2 = self.bound(v1).next;
            let v3 = self.bound(v2).next;
            let (Some(e1), Some(_), Some(e3)) = (
                self.bound(v1).ebev,
                self.bound(v2).ebev,
                self.bound(v3).ebev,
            ) else {
                continue;
            };
            let dir1 = normalize(here - self.position(self.other_vert(self.half(e1).edge, vert)));
            let dir3 = normalize(self.position(self.other_vert(self.half(e3).edge, vert)) - here);
            if angle_normalized(dir1, dir3) < EPSILON_ANG {
                found = Some((v1, dir1));
                break;
            }
        }
        let (v1, dir1) = found?;
        for half in self.bev(bev).halves() {
            if let Some(face) = self.half(half).fnext
                && dir1.dot(self.face_normal(face)).abs() > EPSILON_BIG
            {
                return None;
            }
        }
        Some(v1)
    }

    pub(crate) fn adj_vmesh(&mut self, bev: BevId) -> Grid {
        let n_bndv = self.vmesh(bev).count;
        if n_bndv == 3
            && self.tri_corner_test(bev) != -1
            && self.params.pro_super_r != PRO_SQUARE_IN_R
        {
            return self.tri_corner_adj_vmesh(bev);
        }
        let nseg = self.vmesh(bev).seg;
        let boundstart = self.vmesh(bev).boundstart;
        let mut vm0 = Grid::new(n_bndv, 2, boundstart);
        let mut center = Vec3::ZERO;
        let ring = self.bev_ring(bev);
        for (i, &bound) in ring.iter().enumerate() {
            let co = self.bound(bound).co;
            vm0.set_co(i, 0, 0, co);
            let mid = self.profile_point(&self.bound(bound).profile, 1, 2);
            vm0.set_co(i, 0, 1, mid);
            center += co;
        }
        center *= 1.0 / n_bndv as f32;
        let original = self.position(self.bev(bev).vert);
        let mut negative_fullest = center - original;
        negative_fullest += center;
        let mut fullness = self.params.fullness;
        let direction = original - center;
        let middle = if direction.length_squared() > EPSILON_SQ {
            match self.params.is_custom() {
                true => {
                    fullness *= 2.0;
                    negative_fullest + direction * fullness
                }
                false => center + direction * fullness,
            }
        } else {
            center
        };
        vm0.set_co(0, 1, 1, middle);
        vm0.copy_equivalent();
        let mut vm1 = vm0;
        loop {
            vm1 = self.cubic_subdiv(&mut vm1);
            if vm1.seg >= nseg {
                break;
            }
        }
        if vm1.seg != nseg {
            vm1 = self.interp_vmesh(&vm1, nseg);
        }
        vm1
    }

    fn cube_corner_bounds(&mut self, grid: &mut Grid) {
        grid.count = 0;
        for i in 0..3 {
            let mut co = Vec3::ZERO;
            co[i] = 1.0;
            self.add_bound(grid, co);
        }
    }

    fn cube_corner_square(&mut self, nseg: usize) -> Grid {
        let ns2 = nseg / 2;
        let mut grid = Grid::new(3, nseg, None);
        self.cube_corner_bounds(&mut grid);
        for i in 0..3 {
            for j in 0..=ns2 {
                for k in 0..=ns2 {
                    if !grid.is_canon(i, j, k) {
                        continue;
                    }
                    let mut co = Vec3::ZERO;
                    co[i] = 1.0;
                    co[(i + 1) % 3] = k as f32 * 2.0 / nseg as f32;
                    co[(i + 2) % 3] = j as f32 * 2.0 / nseg as f32;
                    grid.set_co(i, j, k, co);
                }
            }
        }
        grid.copy_equivalent();
        grid
    }

    fn cube_corner_square_in(&mut self, nseg: usize) -> Grid {
        let ns2 = nseg / 2;
        let odd = nseg % 2 == 1;
        let mut grid = Grid::new(3, nseg, None);
        self.cube_corner_bounds(&mut grid);
        let b = match odd {
            true => 2.0 / (2.0 * ns2 as f32 + std::f32::consts::SQRT_2),
            false => 2.0 / nseg as f32,
        };
        for i in 0..3 {
            for k in 0..=ns2 {
                let mut co = Vec3::ZERO;
                co[i] = 1.0 - k as f32 * b;
                grid.set_co(i, 0, k, co);
                let mut co = Vec3::ZERO;
                co[(i + 1) % 3] = 1.0 - k as f32 * b;
                grid.set_co(i, 0, nseg - k, co);
            }
        }
        grid
    }

    fn cube_corner_adj_vmesh(&mut self) -> Grid {
        let nseg = self.params.seg;
        let r = self.params.pro_super_r;
        if !self.params.is_custom() {
            if r == PRO_SQUARE_R {
                return self.cube_corner_square(nseg);
            }
            if r == PRO_SQUARE_IN_R {
                return self.cube_corner_square_in(nseg);
            }
        }
        let mut vm0 = Grid::new(3, 2, None);
        self.cube_corner_bounds(&mut vm0);
        let start = vm0.boundstart.expect("cube corner has a boundary");
        let mut bound = start;
        for i in 0..3 {
            let mut middle = Vec3::ZERO;
            middle[i] = 1.0;
            middle[(i + 1) % 3] = 1.0;
            let start_co = self.bound(bound).co;
            let end_co = self.bound(self.bound(bound).next).co;
            {
                let profile = &mut self.bound_mut(bound).profile;
                profile.super_r = r;
                profile.start = start_co;
                profile.end = end_co;
                profile.middle = middle;
                profile.plane_co = start_co;
                profile.plane_no = start_co.cross(end_co);
                profile.proj_dir = profile.plane_no;
            }
            vm0.set_co(i, 0, 0, start_co);
            self.calculate_profile(bound, false, false);
            let mid = self.profile_point(&self.bound(bound).profile, 1, 2);
            vm0.set_co(i, 0, 1, mid);
            bound = self.bound(bound).next;
        }
        let mut center = Vec3::splat(FRAC_1_SQRT_3);
        if nseg > 2 {
            if r > 1.5 {
                center *= 1.4;
            } else if r < 0.75 {
                center *= 0.6;
            }
        }
        vm0.set_co(0, 1, 1, center);
        vm0.copy_equivalent();
        let mut vm1 = vm0;
        while vm1.seg < nseg {
            vm1 = self.cubic_subdiv(&mut vm1);
        }
        if vm1.seg != nseg {
            vm1 = self.interp_vmesh(&vm1, nseg);
        }
        let ns2 = nseg / 2;
        for i in 0..3 {
            for j in 0..=ns2 {
                for k in 0..=nseg {
                    let co = snap_to_superellipsoid(vm1.co(i, j, k), r, false);
                    vm1.set_co(i, j, k, co);
                }
            }
        }
        vm1
    }

    pub(crate) fn tri_corner_test(&self, bev: BevId) -> i32 {
        if self.params.affect == Affect::Vertices || self.params.is_custom() {
            return -1;
        }
        if self.vmesh(bev).count != 3 {
            return 0;
        }
        let state = self.bev(bev);
        let offset = self.half(state.half(0)).offset_l;
        let mut total = 0.0f32;
        let mut in_plane = 0;
        for half in state.halves() {
            let entry = self.half(half);
            let angle = self.mesh.edge_face_angle_signed(entry.edge).unwrap_or(0.0);
            let absolute = angle.abs();
            if f64::from(absolute) <= std::f64::consts::FRAC_PI_4 {
                in_plane += 1;
            } else if absolute >= 3.0 * FRAC_PI_4 {
                return -1;
            }
            if entry.is_bev && !compare(entry.offset_l, offset, EPSILON) {
                return -1;
            }
            total += angle;
        }
        if in_plane + 3 != state.edgecount {
            return -1;
        }
        let difference = (total.abs() - 3.0 * FRAC_PI_2).abs();
        if (self.params.pro_super_r == PRO_SQUARE_R && difference > PI / 16.0)
            || difference > FRAC_PI_4
        {
            return -1;
        }
        if state.edgecount != 3 || state.selcount != 3 {
            return 0;
        }
        1
    }

    pub(crate) fn tri_corner_adj_vmesh(&mut self, bev: BevId) -> Grid {
        let ring = self.bev_ring(bev);
        let co0 = self.bound(ring[0]).co;
        let co1 = self.bound(ring[1]).co;
        let co2 = self.bound(ring[2]).co;
        let map = Affine::unit_cube(co0, co1, co2, self.position(self.bev(bev).vert));
        let ns = self.params.seg;
        let ns2 = ns / 2;
        let mut grid = self.cube_corner_adj_vmesh();
        for i in 0..3 {
            for j in 0..=ns2 {
                for k in 0..=ns {
                    let co = map.transform(grid.co(i, j, k));
                    grid.set_co(i, j, k, co);
                }
            }
        }
        grid
    }

    fn snap_to_pipe_profile(&self, vpipe: BoundId, midline: bool, co: Vec3) -> Vec3 {
        let profile = &self.bound(vpipe).profile;
        if compare_vec3(profile.start, profile.end, EPSILON_D as f32) {
            return profile.start;
        }
        let half = self.bound(vpipe).ebev.expect("pipe bound is beveled");
        let [v1, v2] = self.mesh.edge_verts(self.half(half).edge);
        let edge_dir = self.position(v1) - self.position(v2);
        let plane = Plane::new(co, edge_dir);
        let start = plane.closest(profile.start);
        let end = plane.closest(profile.end);
        let middle = plane.closest(profile.middle);
        if let Some(map) = Affine::unit_square(start, middle, end)
            && let Some(inverse) = map.inverse()
        {
            let point = inverse.transform(co);
            let snapped = snap_to_superellipsoid(point, profile.super_r, midline);
            return map.transform(snapped);
        }
        closest_to_segment(co, start, end)
    }

    pub(crate) fn pipe_adj_vmesh(&mut self, bev: BevId, vpipe: BoundId) -> Grid {
        let mut grid = self.adj_vmesh(bev);
        let n_bndv = self.vmesh(bev).count;
        let ns = self.vmesh(bev).seg;
        let half_ns = ns / 2;
        let ipipe1 = self.bound(vpipe).index;
        let ipipe2 = self.bound(self.bound(self.bound(vpipe).next).next).index;
        let custom = self.params.is_custom();
        for i in 0..n_bndv {
            for j in 1..=half_ns {
                for k in 0..=half_ns {
                    if !grid.is_canon(i, j, k) {
                        continue;
                    }
                    let co = if custom {
                        let (pipe1, pipe2, factor) = if i == ipipe1 || i == ipipe2 {
                            if n_bndv == 3 && i == ipipe1 {
                                let ring = j.max(k);
                                let pipe2 = grid.co(i, 0, ring);
                                let pipe1 = grid.co(i, ring, 0);
                                let numerator = match k < j {
                                    true => j.min(k) as f32,
                                    false => 2.0 * ring as f32 - j as f32,
                                };
                                (pipe1, pipe2, numerator / (2.0 * ring as f32))
                            } else {
                                let other = match i == ipipe1 {
                                    true => ipipe2,
                                    false => ipipe1,
                                };
                                (
                                    grid.co(i, 0, k),
                                    grid.co(other, 0, ns - k),
                                    j as f32 / ns as f32,
                                )
                            }
                        } else {
                            (grid.co(i, j, 0), grid.co(i, j, ns), k as f32 / ns as f32)
                        };
                        lerp(pipe1, pipe2, factor)
                    } else {
                        let even = ns.is_multiple_of(2);
                        let midline = even
                            && k == half_ns
                            && ((i == 0 && j == half_ns) || i == ipipe1 || i == ipipe2);
                        self.snap_to_pipe_profile(vpipe, midline, grid.co(i, j, k))
                    };
                    grid.set_co(i, j, k, co);
                }
            }
        }
        grid
    }

    pub(crate) fn square_out_adj_vmesh(&mut self, bev: BevId) -> Grid {
        let n_bndv = self.vmesh(bev).count;
        let ns = self.vmesh(bev).seg;
        let ns2 = ns / 2;
        let odd = ns % 2;
        let mut ns2inv = 1.0 / ns2 as f32;
        let boundstart = self.vmesh(bev).boundstart;
        let mut grid = Grid::new(n_bndv, ns, boundstart);
        let vert = self.bev(bev).vert;
        let here = self.position(vert);
        let mut centerline = vec![vec![Vec3::ZERO; ns2 + 1]; n_bndv];
        let mut cset = vec![false; n_bndv];
        let ring = self.bev_ring(bev);
        let mut i = 0;
        while i < n_bndv {
            let bound = ring[i];
            let bndco = self.bound(bound).co;
            let (e1, e2) = (self.bound(bound).efirst, self.bound(bound).elast);
            let angle_kind = match (e1, e2) {
                (Some(e1), Some(e2)) => self.angle_kind(e1, e2, vert),
                _ => AngleKind::Straight,
            };
            if self.bound(bound).is_patch_start {
                let next = self.bound(bound).next;
                centerline[i][0] = midpoint(bndco, self.bound(next).co);
                cset[i] = true;
                i += 1;
                if i < n_bndv {
                    let nextnext = self.bound(next).next;
                    centerline[i][0] = midpoint(self.bound(next).co, self.bound(nextnext).co);
                    cset[i] = true;
                }
                i += 2;
                continue;
            } else if self.bound(bound).is_arc_start {
                centerline[i][0] = self.bound(bound).profile.middle;
                cset[i] = true;
                i += 2;
                continue;
            } else if angle_kind == AngleKind::Smaller
                && let (Some(e1), Some(e2)) = (e1, e2)
            {
                let [a1, a2] = self.mesh.edge_verts(self.half(e1).edge);
                let [b1, b2] = self.mesh.edge_verts(self.half(e2).edge);
                let (pa1, pa2, pb1, pb2) = (
                    self.position(a1),
                    self.position(a2),
                    self.position(b1),
                    self.position(b2),
                );
                let dir1 = pa1 - pa2;
                let dir2 = pb1 - pb2;
                let co1 = bndco + dir1;
                let co2 = bndco + dir2;
                let v1co = line_line(pa1, pa2, bndco, co2).first();
                let v2co = line_line(pb1, pb2, bndco, co1).first();
                let iprev = match i == 0 {
                    true => n_bndv - 1,
                    false => i - 1,
                };
                if let Some(v2co) = v2co {
                    centerline[i][0] = match cset[i] {
                        true => closer(centerline[i][0], v2co, here),
                        false => v2co,
                    };
                    cset[i] = true;
                }
                if let Some(v1co) = v1co {
                    centerline[iprev][0] = match cset[iprev] {
                        true => closer(centerline[iprev][0], v1co, here),
                        false => v1co,
                    };
                    cset[iprev] = true;
                }
            }
            i += 1;
        }
        for (i, &bound) in ring.iter().enumerate() {
            if cset[i] {
                continue;
            }
            let next = self.bound(bound).next;
            let e1 = self.bound(next).efirst;
            let co1 = self.bound(bound).co;
            let co2 = self.bound(next).co;
            if let Some(e1) = e1 {
                let [a1, a2] = self.mesh.edge_verts(self.half(e1).edge);
                let (pa1, pa2) = (self.position(a1), self.position(a2));
                let prev_arc = self.bound(self.bound(bound).prev).is_arc_start;
                let next_arc = self.bound(next).is_arc_start;
                if prev_arc && next_arc {
                    if let Some(meet) = line_line(pa1, pa2, co1, co2).first() {
                        centerline[i][0] = meet;
                        cset[i] = true;
                    }
                } else {
                    centerline[i][0] = match prev_arc {
                        true => closest_to_segment(co1, pa1, pa2),
                        false => closest_to_segment(co2, pa1, pa2),
                    };
                    cset[i] = true;
                }
            }
            if !cset[i] {
                centerline[i][0] = midpoint(co1, co2);
                cset[i] = true;
            }
        }
        let mut co1 = Vec3::ZERO;
        let co2 = here;
        for (i, &bound) in ring.iter().enumerate() {
            if odd == 1 {
                let next = self.bound(bound).next;
                let ang = 0.5 * angle_three(self.bound(bound).co, co1, self.bound(next).co);
                let final_fraction = match ang > SMALL_ANG {
                    true => (0.5 / ang.sin()).min(0.8),
                    false => 0.8,
                };
                ns2inv = 1.0 / (ns2 as f32 + final_fraction);
            }
            co1 = centerline[i][0];
            for (j, point) in centerline[i].iter_mut().enumerate().skip(1) {
                *point = lerp(co1, co2, j as f32 * ns2inv);
            }
        }
        for (i, &bound) in ring.iter().enumerate() {
            let co1 = self.bound(bound).co;
            let previous = match i == 0 {
                true => n_bndv - 1,
                false => i - 1,
            };
            let co2 = centerline[previous][0];
            for j in 0..ns2 + odd {
                grid.set_co(i, j, 0, lerp(co1, co2, j as f32 * ns2inv));
            }
            let co2 = centerline[i][0];
            for k in 1..=ns2 {
                grid.set_co(i, 0, k, lerp(co1, co2, k as f32 * ns2inv));
            }
        }
        if odd == 0 {
            grid.set_co(0, ns2, ns2, here);
        }
        grid.copy_equivalent();
        for i in 0..n_bndv {
            let im1 = match i == 0 {
                true => n_bndv - 1,
                false => i - 1,
            };
            for j in 1..ns2 + odd {
                for k in 1..=ns2 {
                    let meet = line_line(
                        grid.co(i, 0, k),
                        centerline[im1][k],
                        grid.co(i, j, 0),
                        centerline[i][j],
                    );
                    let co = match meet {
                        LineMeet::Parallel => {
                            lerp(grid.co(i, 0, k), centerline[im1][k], j as f32 * ns2inv)
                        }
                        LineMeet::Point(point) => point,
                        LineMeet::Skew(a, b) => midpoint(a, b),
                    };
                    grid.set_co(i, j, k, co);
                }
            }
        }
        grid.copy_equivalent();
        grid
    }
}
