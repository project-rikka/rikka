use rikka_mesh_edit::EdgeId;

use crate::math::{compare, normalize, small_angle_dot};
use crate::model::{BevId, HalfId};
use crate::state::Bevel;

impl Bevel<'_> {
    fn next_beveled_half(
        &self,
        start: HalfId,
        toward: bool,
        bev: &mut Option<BevId>,
    ) -> Option<HalfId> {
        if !toward {
            let other = self.other_end(start);
            *bev = other.map(|half| self.half(half).owner);
            return other;
        }
        let current = (*bev)?;
        let selcount = self.bev(current).selcount;
        if selcount == 1 {
            return None;
        }
        if selcount == 2 {
            let mut half = start;
            loop {
                half = self.next_half(half);
                if self.half(half).is_bev {
                    return Some(half);
                }
            }
        }
        let vert = self.bev(current).vert;
        let [v1, v2] = self.mesh.edge_verts(self.half(start).edge);
        let into = match v1 == vert {
            true => self.position(v1) - self.position(v2),
            false => self.position(v2) - self.position(v1),
        };
        let into = normalize(into);
        let mut best_dot = 0.0f32;
        let mut second_dot = 0.0f32;
        let mut best = None;
        let mut candidate = self.next_half(start);
        while candidate != start {
            if self.half(candidate).is_bev {
                let [c1, c2] = self.mesh.edge_verts(self.half(candidate).edge);
                let out = match c2 == vert {
                    true => self.position(c1) - self.position(c2),
                    false => self.position(c2) - self.position(c1),
                };
                let dot = normalize(out).dot(into);
                if dot > best_dot {
                    second_dot = best_dot;
                    best_dot = dot;
                    best = Some(candidate);
                } else if dot > second_dot {
                    second_dot = dot;
                }
            }
            candidate = self.next_half(candidate);
        }
        if best.is_some() && compare(best_dot, second_dot, small_angle_dot()) {
            return None;
        }
        best
    }

    pub(crate) fn regularize_profile_orientation(&mut self, edge: EdgeId) {
        let [v1, _] = self.mesh.edge_verts(edge);
        let Some(start_bev) = self.bev_of(v1) else {
            return;
        };
        let Some(start) = self.find_half(start_bev, edge) else {
            return;
        };
        if !self.half(start).is_bev || self.half(start).visited_rpo {
            return;
        }
        let (Some(left), Some(right)) = (self.half(start).leftv, self.half(start).rightv) else {
            return;
        };
        let right_highest = self.bound(left).co.z < self.bound(right).co.z;
        self.bound_mut(left).is_profile_start = right_highest;
        self.half_mut(start).visited_rpo = true;
        for pass in 0..2 {
            let mut bev = Some(start_bev);
            let mut toward = pass == 0;
            let mut current = self.next_beveled_half(start, toward, &mut bev);
            while let Some(half) = current {
                if self.half(half).visited_rpo {
                    break;
                }
                if let Some(left) = self.half(half).leftv {
                    let start_side = match pass == 0 {
                        true => toward ^ right_highest,
                        false => (!toward) ^ right_highest,
                    };
                    self.bound_mut(left).is_profile_start = start_side;
                }
                toward = !toward;
                self.half_mut(half).visited_rpo = true;
                current = self.next_beveled_half(half, toward, &mut bev);
            }
        }
    }
}
