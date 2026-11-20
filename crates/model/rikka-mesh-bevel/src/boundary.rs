use glam::Vec3;

use crate::math::{EPSILON_BIG, Plane, line_plane, normalize};
use crate::model::{BevId, BoundId, HalfId, MeshKind};
use crate::offset::AngleKind;
use crate::options::{Affect, InnerMiter, Intersection, OuterMiter, WidthType};
use crate::state::Bevel;

impl Bevel<'_> {
    fn place_bound(
        &mut self,
        bev: BevId,
        construct: bool,
        existing: Option<BoundId>,
        co: Vec3,
    ) -> BoundId {
        match (construct, existing) {
            (false, Some(bound)) => {
                self.bound_mut(bound).co = co;
                bound
            }
            _ => self.add_bev_bound(bev, co),
        }
    }

    pub(crate) fn next_bev(&self, bev: BevId, from: Option<HalfId>) -> Option<HalfId> {
        let state = self.bev(bev);
        let from = from.unwrap_or_else(|| state.half(state.edgecount - 1));
        let mut half = from;
        loop {
            if self.half(half).is_bev {
                return Some(half);
            }
            half = self.next_half(half);
            if half == from {
                return None;
            }
        }
    }

    pub(crate) fn build_boundary(&mut self, bev: BevId, construct: bool) {
        if self.bev(bev).edgecount <= 1 {
            return;
        }
        if self.params.affect == Affect::Vertices {
            self.build_boundary_vertex_only(bev, construct);
            return;
        }
        let Some(efirst) = self.next_bev(bev, None) else {
            return;
        };
        if self.bev(bev).selcount == 1 {
            self.build_boundary_terminal_edge(bev, efirst, construct);
            return;
        }
        let vert = self.bev(bev).vert;
        let miter_outer = match self.bev(bev).selcount >= 3 {
            true => self.params.miter_outer,
            false => OuterMiter::Sharp,
        };
        let miter_inner = self.params.miter_inner;
        let mut emiter: Option<HalfId> = None;
        let mut e = efirst;
        loop {
            let mut in_plane = 0;
            let mut not_in_plane = 0;
            let mut enip = None;
            let mut eip = None;
            let mut e2 = self.next_half(e);
            while !self.half(e2).is_bev {
                match self.eh_on_plane(e2) {
                    true => {
                        in_plane += 1;
                        eip = Some(e2);
                    }
                    false => {
                        not_in_plane += 1;
                        enip = Some(e2);
                    }
                }
                e2 = self.next_half(e2);
            }
            let mut eon = None;
            let mut sinratio = 1.0;
            let co = if in_plane == 0 && not_in_plane == 0 {
                self.offset_meet(e, e2, vert, self.half(e).fnext, false, None)
            } else if not_in_plane > 0 {
                let enip = enip.expect("not in plane edge was counted");
                if self.params.loop_slide
                    && not_in_plane == 1
                    && self.good_offset_on_edge_between(e, e2, enip, vert)
                {
                    let meet = self.offset_on_edge_between(e, e2, enip, vert);
                    if let Some(ratio) = meet.sinratio {
                        eon = Some(enip);
                        sinratio = ratio;
                    }
                    meet.point
                } else {
                    self.offset_meet(e, e2, vert, None, true, eip)
                }
            } else {
                let eip = eip.expect("in plane edge was counted");
                if self.params.loop_slide
                    && in_plane == 1
                    && self.good_offset_on_edge_between(e, e2, eip, vert)
                {
                    let meet = self.offset_on_edge_between(e, e2, eip, vert);
                    if let Some(ratio) = meet.sinratio {
                        eon = Some(eip);
                        sinratio = ratio;
                    }
                    meet.point
                } else {
                    self.offset_meet(e, e2, vert, self.half(e).fnext, false, None)
                }
            };
            let angle_kind = self.angle_kind(e, e2, vert);
            let special = (miter_outer != OuterMiter::Sharp
                && emiter.is_none()
                && angle_kind == AngleKind::Larger)
                || (miter_inner != InnerMiter::Sharp && angle_kind == AngleKind::Smaller);
            if construct {
                let v = self.add_bev_bound(bev, co);
                {
                    let bound = self.bound_mut(v);
                    bound.efirst = Some(e);
                    bound.elast = Some(e2);
                    bound.ebev = Some(e2);
                    bound.eon = eon;
                    if eon.is_some() {
                        bound.sinratio = sinratio;
                    }
                }
                self.half_mut(e).rightv = Some(v);
                self.half_mut(e2).leftv = Some(v);
                let mut e3 = self.next_half(e);
                while e3 != e2 {
                    self.half_mut(e3).leftv = Some(v);
                    self.half_mut(e3).rightv = Some(v);
                    e3 = self.next_half(e3);
                }
                if special {
                    if angle_kind == AngleKind::Larger {
                        emiter = Some(e);
                    }
                    self.bound_mut(v).ebev = None;
                    let patch = angle_kind == AngleKind::Larger && miter_outer == OuterMiter::Patch;
                    let v2 = patch.then(|| self.add_bev_bound(bev, co));
                    let v3 = self.add_bev_bound(bev, co);
                    {
                        let bound = self.bound_mut(v3);
                        bound.ebev = Some(e2);
                        bound.efirst = Some(e2);
                        bound.elast = Some(e2);
                        bound.eon = None;
                    }
                    self.half_mut(e2).leftv = Some(v3);
                    if let Some(v2) = v2 {
                        let (v1_eon, v1_ratio) = (self.bound(v).eon, self.bound(v).sinratio);
                        self.bound_mut(v).is_patch_start = true;
                        {
                            let bound = self.bound_mut(v2);
                            bound.eon = v1_eon;
                            bound.sinratio = v1_ratio;
                            bound.ebev = None;
                        }
                        {
                            let bound = self.bound_mut(v);
                            bound.eon = None;
                            bound.sinratio = 1.0;
                            bound.elast = Some(e);
                        }
                        if self.next_half(e) == e2 {
                            self.bound_mut(v2).efirst = None;
                            self.bound_mut(v2).elast = None;
                        } else {
                            let first = self.next_half(e);
                            self.bound_mut(v2).efirst = Some(first);
                            let mut e3 = first;
                            while e3 != e2 {
                                self.half_mut(e3).leftv = Some(v2);
                                self.half_mut(e3).rightv = Some(v2);
                                self.bound_mut(v2).elast = Some(e3);
                                e3 = self.next_half(e3);
                            }
                        }
                    } else {
                        self.bound_mut(v).is_arc_start = true;
                        self.bound_mut(v).profile.middle = co;
                        if self.next_half(e) == e2 {
                            let first = self.bound(v).efirst;
                            self.bound_mut(v).elast = first;
                        } else {
                            let between = in_plane + not_in_plane;
                            let half_between = between / 2;
                            let odd = between % 2 == 1;
                            let seg = self.params.seg;
                            let mut i = 0;
                            let mut e3 = self.next_half(e);
                            while e3 != e2 {
                                self.bound_mut(v).elast = Some(e3);
                                self.half_mut(e3).profile_index = if i < half_between {
                                    0
                                } else if odd && i == half_between {
                                    seg / 2
                                } else {
                                    seg
                                };
                                i += 1;
                                e3 = self.next_half(e3);
                            }
                        }
                    }
                }
            } else if special {
                if angle_kind == AngleKind::Larger {
                    emiter = Some(e);
                }
                let v1 = self.half(e).rightv.expect("boundary was constructed");
                self.bound_mut(v1).co = co;
                let mut next = self.bound(v1).next;
                if angle_kind == AngleKind::Larger && miter_outer == OuterMiter::Patch {
                    self.bound_mut(next).co = co;
                    next = self.bound(next).next;
                }
                self.bound_mut(next).co = co;
            } else {
                let v = self.half(e).rightv.expect("boundary was constructed");
                self.bound_mut(v).co = co;
            }
            e = e2;
            if e == efirst {
                break;
            }
        }
        if miter_inner != InnerMiter::Sharp {
            self.adjust_miter_inner_coords(bev, emiter);
        }
        if let Some(emiter) = emiter {
            self.adjust_miter_coords(bev, emiter);
        }
        if construct {
            self.set_bound_vert_seams(bev);
            let count = self.vmesh(bev).count;
            let kind = if count == 2 {
                MeshKind::None
            } else if self.half(efirst).seg == 1 {
                MeshKind::Poly
            } else {
                match self.params.vmesh_method {
                    Intersection::GridFill => MeshKind::Adj,
                    Intersection::Cutoff => MeshKind::Cutoff,
                }
            };
            self.vmesh_mut(bev).kind = kind;
        }
    }

    fn build_boundary_vertex_only(&mut self, bev: BevId, construct: bool) {
        let vert = self.bev(bev).vert;
        for half in self.bev(bev).halves() {
            let co = self.slide_dist(self.half(half).edge, vert, self.half(half).offset_l);
            if construct {
                let v = self.add_bev_bound(bev, co);
                self.bound_mut(v).efirst = Some(half);
                self.bound_mut(v).elast = Some(half);
                self.half_mut(half).leftv = Some(v);
                self.half_mut(half).rightv = Some(v);
            } else if let Some(v) = self.half(half).leftv {
                self.bound_mut(v).co = co;
            }
        }
        if construct {
            self.set_bound_vert_seams(bev);
            if self.params.affect_vertices_odd
                && !self.bev(bev).any_seam
                && !self.contig_ldata_around_vert(vert)
            {
                self.bev_mut(bev).any_seam = true;
            }
            let kind = if self.vmesh(bev).count == 2 {
                MeshKind::None
            } else if self.params.seg == 1 {
                MeshKind::Poly
            } else {
                MeshKind::Adj
            };
            self.vmesh_mut(bev).kind = kind;
        }
    }

    fn build_boundary_terminal_edge(&mut self, bev: BevId, efirst: HalfId, construct: bool) {
        let vert = self.bev(bev).vert;
        let edgecount = self.bev(bev).edgecount;
        let mut e = efirst;
        if edgecount == 2 {
            let (fprev, fnext) = (self.half(e).fprev, self.half(e).fnext);
            let normal = fprev.or(fnext).map(|face| self.face_normal(face));
            let co = self.offset_in_plane(e, normal, true);
            let existing = self.half(e).leftv;
            let v = self.place_bound(bev, construct, existing, co);
            if construct {
                let bound = self.bound_mut(v);
                bound.efirst = Some(e);
                bound.elast = Some(e);
                bound.ebev = Some(e);
                self.half_mut(e).leftv = Some(v);
            }
            let normal = fnext.or(fprev).map(|face| self.face_normal(face));
            let co = self.offset_in_plane(e, normal, false);
            let existing = self.half(e).rightv;
            let v = self.place_bound(bev, construct, existing, co);
            if construct {
                self.bound_mut(v).efirst = Some(e);
                self.bound_mut(v).elast = Some(e);
                self.half_mut(e).rightv = Some(v);
            }
            let next = self.next_half(e);
            let co = self.slide_dist(self.half(next).edge, vert, self.half(e).offset_l);
            let existing = self.half(next).leftv;
            let v = self.place_bound(bev, construct, existing, co);
            if construct {
                self.bound_mut(v).efirst = Some(next);
                self.bound_mut(v).elast = Some(next);
                self.half_mut(next).leftv = Some(v);
                self.half_mut(next).rightv = Some(v);
                self.set_bound_vert_seams(bev);
            }
        } else {
            let leg_slide = matches!(
                self.params.offset_type,
                WidthType::Percent | WidthType::Absolute
            );
            let prev = self.prev_half(e);
            let co = match leg_slide {
                true => self.slide_dist(self.half(prev).edge, vert, self.half(e).offset_l),
                false => self.offset_meet(prev, e, vert, self.half(e).fprev, false, None),
            };
            let existing = self.half(e).leftv;
            let v = self.place_bound(bev, construct, existing, co);
            if construct {
                let bound = self.bound_mut(v);
                bound.efirst = Some(prev);
                bound.elast = Some(e);
                bound.ebev = Some(e);
                self.half_mut(e).leftv = Some(v);
                self.half_mut(prev).leftv = Some(v);
                self.half_mut(prev).rightv = Some(v);
            }
            e = self.next_half(e);
            let prev = self.prev_half(e);
            let co = match leg_slide {
                true => self.slide_dist(self.half(e).edge, vert, self.half(prev).offset_r),
                false => self.offset_meet(prev, e, vert, self.half(e).fprev, false, None),
            };
            let existing = self.half(e).leftv;
            let v = self.place_bound(bev, construct, existing, co);
            if construct {
                self.bound_mut(v).efirst = Some(prev);
                self.bound_mut(v).elast = Some(e);
                self.half_mut(e).leftv = Some(v);
                self.half_mut(e).rightv = Some(v);
                self.half_mut(prev).rightv = Some(v);
            }
            let mut distance = self.half(efirst).offset_l_spec;
            if self.params.is_custom() || self.params.profile < 0.25 {
                distance *= 2.0f32.sqrt();
            }
            e = self.next_half(e);
            while self.next_half(e) != efirst {
                let co = self.slide_dist(self.half(e).edge, vert, distance);
                let existing = self.half(e).leftv;
                let v = self.place_bound(bev, construct, existing, co);
                if construct {
                    self.bound_mut(v).efirst = Some(e);
                    self.bound_mut(v).elast = Some(e);
                    self.half_mut(e).leftv = Some(v);
                    self.half_mut(e).rightv = Some(v);
                }
                e = self.next_half(e);
            }
        }
        if edgecount >= 3 {
            let start = self.vmesh(bev).boundstart.expect("boundary was built");
            self.set_profile_params(bev, start);
            let bev_co = self.position(vert);
            self.move_profile_plane(start, bev_co);
        }
        if construct {
            self.set_bound_vert_seams(bev);
            let count = self.vmesh(bev).count;
            let kind = if count == 2 && edgecount == 3 {
                MeshKind::None
            } else if count == 3 {
                let mut tri_fan = true;
                if self.params.is_custom() {
                    let left = self.half(efirst).leftv.expect("boundary was built");
                    let profile = &self.bound(left).profile;
                    let plane = Plane::new(profile.plane_co, profile.plane_no);
                    let right = self.half(efirst).rightv.expect("boundary was built");
                    let extra = self.bound(right).next;
                    if plane.distance_squared(self.bound(extra).co) < EPSILON_BIG {
                        tri_fan = false;
                    }
                }
                match tri_fan {
                    true => MeshKind::TriFan,
                    false => MeshKind::Poly,
                }
            } else {
                MeshKind::Poly
            };
            self.vmesh_mut(bev).kind = kind;
        }
    }

    fn adjust_miter_coords(&mut self, bev: BevId, emiter: HalfId) {
        let vert = self.bev(bev).vert;
        let here = self.position(vert);
        let v1 = self.half(emiter).rightv.expect("boundary was built");
        let v3 = match self.params.miter_outer {
            OuterMiter::Patch => self.bound(self.bound(v1).next).next,
            _ => self.bound(v1).next,
        };
        let v1prev = self.bound(v1).prev;
        let v3next = self.bound(v3).next;
        let co2 = self.bound(v1).co;
        if self.bound(v1).is_arc_start {
            self.bound_mut(v1).profile.middle = co2;
        }
        let vother = self.other_vert(self.half(emiter).edge, vert);
        let mut edge_dir = normalize(here - self.position(vother));
        let distance = self.params.offset / (self.params.seg as f32 / 2.0);
        let mut line_p = co2 + edge_dir * distance;
        let co1 = line_plane(co2, line_p, self.bound(v1prev).co, edge_dir).unwrap_or(line_p);
        self.bound_mut(v1).co = co1;
        let other_half = self.bound(v3).elast.expect("miter boundary has an edge");
        let vother = self.other_vert(self.half(other_half).edge, vert);
        edge_dir = normalize(here - self.position(vother));
        line_p = co2 + edge_dir * distance;
        let co3 = line_plane(co2, line_p, self.bound(v3next).co, edge_dir).unwrap_or(line_p);
        self.bound_mut(v3).co = co3;
    }

    fn adjust_miter_inner_coords(&mut self, bev: BevId, emiter: Option<HalfId>) {
        let vert = self.bev(bev).vert;
        let here = self.position(vert);
        let Some(start) = self.vmesh(bev).boundstart else {
            return;
        };
        let mut v = start;
        loop {
            if self.bound(v).is_arc_start {
                let v3 = self.bound(v).next;
                let e = self.bound(v).efirst;
                if e != emiter
                    && let Some(e) = e
                {
                    let co = self.bound(v).co;
                    let vother = self.other_vert(self.half(e).edge, vert);
                    let edge_dir = normalize(self.position(vother) - here);
                    self.bound_mut(v).co = co + edge_dir * self.params.spread;
                    let e3 = self.bound(v3).elast.expect("miter boundary has an edge");
                    let vother = self.other_vert(self.half(e3).edge, vert);
                    let edge_dir = normalize(self.position(vother) - here);
                    self.bound_mut(v3).co = co + edge_dir * self.params.spread;
                }
                v = self.bound(v3).next;
            } else {
                v = self.bound(v).next;
            }
            if v == start {
                break;
            }
        }
    }

    pub(crate) fn eh_on_plane(&self, half: HalfId) -> bool {
        let state = self.half(half);
        let (Some(prev), Some(next)) = (state.fprev, state.fnext) else {
            return false;
        };
        let dot = self.face_normal(prev).dot(self.face_normal(next));
        (dot + 1.0).abs() <= EPSILON_BIG || (dot - 1.0).abs() <= EPSILON_BIG
    }
}
