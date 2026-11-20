use glam::Vec3;

use crate::math::{
    Affine, EPSILON, EPSILON_BIG, line_line, line_plane, midpoint, nearly_parallel, normalize,
    normalize_with_length,
};
use crate::model::{BevId, BoundId, Profile};
use crate::options::Affect;
use crate::spacing::{PRO_CIRCLE_R, PRO_LINE_R, PRO_SQUARE_IN_R, PRO_SQUARE_R, Samples};
use crate::state::Bevel;

impl Bevel<'_> {
    pub(crate) fn set_profile_params(&mut self, bev: BevId, bound: BoundId) {
        let start = self.bound(bound).co;
        let end = self.bound(self.bound(bound).next).co;
        let ebev = self.bound(bound).ebev;
        let is_arc_start = self.bound(bound).is_arc_start;
        let bev_vert = self.bev(bev).vert;
        let bev_co = self.position(bev_vert);
        let mut profile = std::mem::take(&mut self.bound_mut(bound).profile);
        let mut linear = true;
        if let Some(half) = ebev {
            linear = false;
            let state = self.half(half).clone();
            profile.super_r = self.params.pro_super_r;
            let [v1, v2] = self.mesh.edge_verts(state.edge);
            let mut proj_dir = self.position(v1) - self.position(v2);
            if state.is_rev {
                proj_dir = -proj_dir;
            }
            profile.proj_dir = normalize(proj_dir);
            profile.middle = self.project_to_edge(state.edge, start, end);
            profile.start = start;
            profile.end = end;
            let mut d1 = normalize(profile.middle - start);
            let mut d2 = normalize(profile.middle - end);
            profile.plane_no = normalize(d1.cross(d2));
            if nearly_parallel(d1, d2) {
                profile.middle = bev_co;
                let prev = self.prev_half(half);
                let next = self.next_half(half);
                if self.half(prev).is_bev && self.half(next).is_bev && self.bev(bev).selcount >= 3 {
                    let [p1, p2] = self.mesh.edge_verts(self.half(prev).edge);
                    let [n1, n2] = self.mesh.edge_verts(self.half(next).edge);
                    let d3 = normalize(self.position(p1) - self.position(p2));
                    let d4 = normalize(self.position(n1) - self.position(n2));
                    if nearly_parallel(d3, d4) {
                        profile.middle = midpoint(start, end);
                        linear = true;
                    } else {
                        match line_line(start, start + d3, end, end + d4).first() {
                            Some(meet) => profile.middle = meet,
                            None => {
                                profile.middle = midpoint(start, end);
                                linear = true;
                            }
                        }
                    }
                }
                profile.end = end;
                d1 = normalize(profile.middle - start);
                d2 = normalize(profile.middle - end);
                profile.plane_no = normalize(d1.cross(d2));
                if nearly_parallel(d1, d2) {
                    linear = true;
                } else {
                    profile.plane_co = bev_co;
                    profile.proj_dir = profile.plane_no;
                }
            }
            profile.plane_co = start;
        } else if is_arc_start {
            profile.start = start;
            profile.end = end;
            profile.super_r = PRO_CIRCLE_R;
            profile.plane_co = Vec3::ZERO;
            profile.plane_no = Vec3::ZERO;
            profile.proj_dir = Vec3::ZERO;
            linear = false;
        } else if self.params.affect == Affect::Vertices {
            profile.start = start;
            profile.middle = bev_co;
            profile.end = end;
            profile.super_r = self.params.pro_super_r;
            profile.plane_co = Vec3::ZERO;
            profile.plane_no = Vec3::ZERO;
            profile.proj_dir = Vec3::ZERO;
            linear = false;
        }
        if linear {
            profile.super_r = PRO_LINE_R;
            profile.start = start;
            profile.end = end;
            profile.middle = midpoint(start, end);
            profile.plane_co = Vec3::ZERO;
            profile.plane_no = Vec3::ZERO;
            profile.proj_dir = Vec3::ZERO;
        }
        self.bound_mut(bound).profile = profile;
    }

    pub(crate) fn move_profile_plane(&mut self, bound: BoundId, bev_co: Vec3) {
        let profile = &mut self.bound_mut(bound).profile;
        if profile.proj_dir == Vec3::ZERO {
            return;
        }
        let d1 = normalize(bev_co - profile.start);
        let d2 = normalize(bev_co - profile.end);
        let (no, l1) = normalize_with_length(d1.cross(d2));
        let (no2, l2) = normalize_with_length(d1.cross(profile.proj_dir));
        let (no3, l3) = normalize_with_length(d2.cross(profile.proj_dir));
        if l1 > EPSILON_BIG && l2 > EPSILON_BIG && l3 > EPSILON_BIG {
            let dot2 = no.dot(no2);
            let dot3 = no.dot(no3);
            if dot2.abs() < (1.0 - EPSILON_BIG) && dot3.abs() < (1.0 - EPSILON_BIG) {
                profile.plane_no = no;
            }
        }
        profile.special_params = true;
    }

    pub(crate) fn move_weld_profile_planes(&mut self, bev_co: Vec3, b1: BoundId, b2: BoundId) {
        if self.bound(b1).profile.proj_dir == Vec3::ZERO
            || self.bound(b2).profile.proj_dir == Vec3::ZERO
        {
            return;
        }
        let d1 = bev_co - self.bound(b1).co;
        let d2 = bev_co - self.bound(b2).co;
        let (no, l1) = normalize_with_length(d1.cross(d2));
        let (no2, l2) = normalize_with_length(d1.cross(self.bound(b1).profile.proj_dir));
        let (no3, l3) = normalize_with_length(d2.cross(self.bound(b2).profile.proj_dir));
        if l1 != 0.0 && (l2 != 0.0 || l3 != 0.0) {
            let dot1 = no.dot(no2).abs();
            let dot2 = no.dot(no3).abs();
            if (dot1 - 1.0).abs() > EPSILON {
                self.bound_mut(b1).profile.plane_no = no;
            }
            if (dot2 - 1.0).abs() > EPSILON {
                self.bound_mut(b2).profile.plane_no = no;
            }
        }
        self.bound_mut(b1).profile.special_params = true;
        self.bound_mut(b2).profile.special_params = true;
    }

    pub(crate) fn calculate_profile(&mut self, bound: BoundId, reversed: bool, miter: bool) {
        if self.params.seg == 1 {
            return;
        }
        let mut profile = std::mem::take(&mut self.bound_mut(bound).profile);
        self.fill_profile(&mut profile, reversed, miter);
        self.bound_mut(bound).profile = profile;
    }

    pub(crate) fn fill_profile(&self, profile: &mut Profile, reversed: bool, miter: bool) {
        let seg = self.params.seg;
        let seg_2 = self.params.seg_2;
        let spacing = match (miter, &self.params.spacing_miter) {
            (true, Some(spacing)) => spacing,
            _ => &self.params.spacing,
        };
        let map = match !self.params.is_custom() && profile.super_r == PRO_LINE_R {
            true => None,
            false => Affine::unit_square(profile.start, profile.middle, profile.end),
        };
        if self.params.vmesh_method == crate::options::Intersection::Cutoff
            && let Some(map) = map
        {
            let bottom = map.transform(Vec3::ZERO);
            let top = map.transform(Vec3::new(1.0, 1.0, 0.0));
            profile.height = bottom.distance(top);
        }
        profile.points = profile_segments(profile, map, reversed, seg, &spacing.main);
        profile.power_points = (seg != seg_2)
            .then(|| profile_segments(profile, map, reversed, seg_2, spacing.power_samples()));
    }

    pub(crate) fn profile_point(&self, profile: &Profile, i: usize, nseg: usize) -> Vec3 {
        if self.params.seg == 1 {
            return match i == 0 {
                true => profile.start,
                false => profile.end,
            };
        }
        if nseg == self.params.seg {
            return profile.points[i];
        }
        let spacing = self.params.seg_2 / nseg;
        profile.power_points()[i * spacing]
    }

    pub(crate) fn calculate_vm_profiles(&mut self, bev: BevId) {
        for bound in self.bev_ring(bev) {
            if !self.bound(bound).profile.special_params {
                self.set_profile_params(bev, bound);
            }
            let (mut miter, mut reverse) = (false, false);
            if self.params.is_custom() {
                let state = self.bound(bound);
                miter = state.is_arc_start || state.is_patch_start;
                reverse = !state.is_profile_start && !miter;
            }
            self.calculate_profile(bound, reverse, miter);
        }
    }
}

fn profile_segments(
    profile: &Profile,
    map: Option<Affine>,
    reversed: bool,
    segments: usize,
    samples: &Samples,
) -> Vec<Vec3> {
    (0..=segments)
        .map(|k| {
            let co = if k == 0 {
                profile.start
            } else if k == segments {
                profile.end
            } else {
                match map {
                    Some(map) => {
                        let point = match reversed {
                            true => Vec3::new(
                                samples.y[segments - k] as f32,
                                samples.x[segments - k] as f32,
                                0.0,
                            ),
                            false => Vec3::new(samples.x[k] as f32, samples.y[k] as f32, 0.0),
                        };
                        map.transform(point)
                    }
                    None => {
                        crate::math::lerp(profile.start, profile.end, k as f32 / segments as f32)
                    }
                }
            };
            if profile.proj_dir != Vec3::ZERO {
                line_plane(
                    co,
                    co + profile.proj_dir,
                    profile.plane_co,
                    profile.plane_no,
                )
                .unwrap_or(co)
            } else {
                co
            }
        })
        .collect()
}

pub(crate) fn snap_to_superellipsoid(co: Vec3, super_r: f32, midline: bool) -> Vec3 {
    if super_r == PRO_CIRCLE_R {
        return normalize(co);
    }
    let a = co.x.max(0.0);
    let b = co.y.max(0.0);
    let c = co.z.max(0.0);
    if super_r == PRO_SQUARE_R || super_r == PRO_SQUARE_IN_R {
        let mut x = a.min(1.0);
        let mut y = b.min(1.0);
        if super_r == PRO_SQUARE_R {
            if 1.0 - x < 1.0 - y {
                x = 1.0;
                if midline {
                    y = 1.0;
                }
            } else {
                y = 1.0;
                if midline {
                    x = 1.0;
                }
            }
        } else if x < y {
            x = 0.0;
            if midline {
                y = 0.0;
            }
        } else {
            y = 0.0;
            if midline {
                x = 0.0;
            }
        }
        return Vec3::new(x, y, 0.0);
    }
    let rinv = 1.0 / super_r;
    if a == 0.0 {
        if b == 0.0 {
            Vec3::new(0.0, 0.0, c.powf(rinv))
        } else {
            let y = (1.0 / (1.0 + (c / b).powf(super_r))).powf(rinv);
            Vec3::new(0.0, y, c * y / b)
        }
    } else {
        let x = (1.0 / (1.0 + (b / a).powf(super_r) + (c / a).powf(super_r))).powf(rinv);
        Vec3::new(x, b * x / a, c * x / a)
    }
}
