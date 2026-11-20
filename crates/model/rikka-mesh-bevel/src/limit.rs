use std::f32::consts::PI;

use rikka_mesh_edit::VertId;

use crate::math::{EPSILON, angle_three, safe_divide};
use crate::model::HalfId;
use crate::options::{Affect, WidthType};
use crate::state::Bevel;

const NO_COLLIDE_MARGIN: f32 = 1.0e6;
const MIN_EXTERIOR_ANGLE: f32 = 0.0001;

impl Bevel<'_> {
    fn geometry_collide_offset(&self, eb: HalfId) -> f32 {
        let offset = self.params.offset;
        let no_collide = offset + NO_COLLIDE_MARGIN;
        let mut limit = no_collide;
        if offset == 0.0 {
            return no_collide;
        }
        let state = self.half(eb);
        let mut kb = state.offset_l_spec;
        let ea = self.next_half(eb);
        let ka = self.half(ea).offset_r_spec;
        let [v1, v2] = self.mesh.edge_verts(state.edge);
        let (vb, mut vc) = match state.is_rev {
            true => (v2, v1),
            false => (v1, v2),
        };
        let va = self.half_other_vert(ea);
        let ebother = self.other_end(eb);
        if self.params.uses_percent_or_absolute() {
            let other_prev_bev = ebother
                .map(|other| self.half(self.prev_half(other)).is_bev)
                .unwrap_or(false);
            if self.half(ea).is_bev && other_prev_bev {
                if self.params.offset_type == WidthType::Percent {
                    return 50.0;
                }
                let length = self.mesh.edge_length(state.edge);
                return match offset > length / 2.0 {
                    true => length / 2.0,
                    false => length,
                };
            }
            return no_collide;
        }
        let (ec, vd, kc) = match ebother {
            Some(other) => {
                let ec = self.prev_half(other);
                vc = self.half_vert(other);
                (
                    Some(ec),
                    self.half_other_vert(ec),
                    self.half(ec).offset_l_spec,
                )
            }
            None => {
                let Some(fnext) = state.fnext else {
                    return no_collide;
                };
                let Some(lb) = self.mesh.face_edge_share_loop(fnext, state.edge) else {
                    return no_collide;
                };
                let vd = if self.mesh.loop_next_vert(lb) == vc {
                    self.mesh.loop_next_vert(self.mesh.loop_next(lb))
                } else if self.mesh.loop_vert(lb) == vc {
                    self.mesh.loop_vert(self.mesh.loop_prev(lb))
                } else {
                    return no_collide;
                };
                (None, vd, 0.0)
            }
        };
        if self.half(ea).edge == state.edge || ec.is_some_and(|ec| self.half(ec).edge == state.edge)
        {
            return no_collide;
        }
        let (pa, pb, pc, pd) = (
            self.position(va),
            self.position(vb),
            self.position(vc),
            self.position(vd),
        );
        let th1 = angle_three(pa, pb, pc);
        let th2 = angle_three(pb, pc, pd);
        let (sin1, sin2) = (th1.sin(), th2.sin());
        let (cos1, cos2) = (th1.cos(), th2.cos());
        let mut projected = safe_divide(ka + cos1 * kb, sin1) + safe_divide(kc + cos2 * kb, sin2);
        if projected > EPSILON {
            projected = offset * (pb.distance(pc) / projected);
            if projected > EPSILON {
                limit = projected;
            }
        }
        if kb > f32::EPSILON && (ka == 0.0 || kc == 0.0) {
            kb = offset / kb;
            let Some(fnext) = state.fnext else {
                return limit;
            };
            if ka == 0.0
                && let Some(mut la) = self.mesh.face_edge_share_loop(fnext, self.half(ea).edge)
            {
                let mut slide = 0.0f32;
                let mut exterior = 0.0f32;
                let mut first = true;
                let mut steps = self.mesh.face_len(fnext);
                while exterior < MIN_EXTERIOR_ANGLE && steps > 0 {
                    steps -= 1;
                    if first {
                        exterior = PI - th1;
                        first = false;
                    } else {
                        la = self.mesh.loop_prev(la);
                        let next = self.mesh.loop_next(la);
                        exterior += PI
                            - angle_three(
                                self.position(self.mesh.loop_vert(la)),
                                self.position(self.mesh.loop_vert(next)),
                                self.position(self.mesh.loop_vert(self.mesh.loop_next(next))),
                            );
                    }
                    slide += self.mesh.edge_length(self.mesh.loop_edge(la)) * exterior.sin();
                }
                limit = limit.min(slide * kb);
            }
            if kc == 0.0
                && let Some(lc) = self.mesh.face_edge_share_loop(fnext, state.edge)
            {
                let mut lc = self.mesh.loop_next(lc);
                let mut slide = 0.0f32;
                let mut exterior = 0.0f32;
                let mut first = true;
                let mut steps = self.mesh.face_len(fnext);
                while exterior < MIN_EXTERIOR_ANGLE && steps > 0 {
                    steps -= 1;
                    if first {
                        exterior = PI - th2;
                        first = false;
                    } else {
                        lc = self.mesh.loop_next(lc);
                        exterior += PI
                            - angle_three(
                                self.position(self.mesh.loop_vert(self.mesh.loop_prev(lc))),
                                self.position(self.mesh.loop_vert(lc)),
                                self.position(self.mesh.loop_next_vert(lc)),
                            );
                    }
                    slide += self.mesh.edge_length(self.mesh.loop_edge(lc)) * exterior.sin();
                }
                limit = limit.min(slide * kb);
            }
        }
        limit
    }

    fn vertex_collide_offset(&self, ea: HalfId) -> f32 {
        let offset = self.params.offset;
        let no_collide = offset + NO_COLLIDE_MARGIN;
        if offset == 0.0 {
            return no_collide;
        }
        let ka = self.half(ea).offset_l_spec / offset;
        let kb = self
            .other_end(ea)
            .map_or(0.0, |eb| self.half(eb).offset_l_spec / offset);
        let kab = ka + kb;
        let length = self.mesh.edge_length(self.half(ea).edge);
        match kab <= 0.0 {
            true => no_collide,
            false => length / kab,
        }
    }

    pub(crate) fn limit_offset(&mut self, tagged: &[VertId]) {
        let mut limited = self.params.offset;
        for &vert in tagged {
            let Some(bev) = self.bev_of(vert) else {
                continue;
            };
            for half in self.bev(bev).halves() {
                let collision = match self.params.affect {
                    Affect::Vertices => self.vertex_collide_offset(half),
                    Affect::Edges => self.geometry_collide_offset(half),
                };
                limited = limited.min(collision);
            }
        }
        if limited < self.params.offset {
            let factor = limited / self.params.offset;
            for &vert in tagged {
                let Some(bev) = self.bev_of(vert) else {
                    continue;
                };
                for half in self.bev(bev).halves() {
                    let entry = self.half_mut(half);
                    entry.offset_l_spec *= factor;
                    entry.offset_r_spec *= factor;
                    entry.offset_l *= factor;
                    entry.offset_r *= factor;
                }
            }
            self.params.offset = limited;
        }
    }
}
