use crate::model::{BevId, MeshKind};
use crate::state::Bevel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeMark {
    Seam,
    Sharp,
}

impl Bevel<'_> {
    pub(crate) fn set_bound_vert_seams(&mut self, bev: BevId) {
        let mut bev_any = false;
        for bound in self.bev_ring(bev) {
            let mut any = false;
            let (first, last) = (self.bound(bound).efirst, self.bound(bound).elast);
            if let Some(mut half) = first {
                loop {
                    any |= self.half(half).is_seam;
                    if Some(half) == last {
                        break;
                    }
                    half = self.next_half(half);
                }
            }
            self.bound_mut(bound).any_seam = any;
            bev_any |= any;
        }
        self.bev_mut(bev).any_seam = bev_any;
        if self.params.mark_seam {
            self.count_unmarked_runs(bev, EdgeMark::Seam);
        }
        if self.params.mark_sharp {
            self.count_unmarked_runs(bev, EdgeMark::Sharp);
        }
    }

    fn lacks_mark(&self, half: crate::model::HalfId, mark: EdgeMark) -> bool {
        let edge = self.half(half).edge;
        match mark {
            EdgeMark::Seam => !self.mesh.is_edge_seam(edge),
            EdgeMark::Sharp => !self.mesh.is_edge_sharp(edge),
        }
    }

    fn count_unmarked_runs(&mut self, bev: BevId, mark: EdgeMark) {
        let state = self.bev(bev);
        let mut e = state.half(0);
        let mut efirst = e;
        while self.lacks_mark(e, mark) {
            e = self.next_half(e);
            if e == efirst {
                break;
            }
        }
        if self.lacks_mark(e, mark) {
            return;
        }
        efirst = e;
        loop {
            let mut count = 0;
            let mut ne = self.next_half(e);
            while self.lacks_mark(ne, mark) && ne != efirst {
                if self.half(ne).is_bev {
                    count += 1;
                }
                ne = self.next_half(ne);
            }
            if ne == e || (ne == efirst && self.lacks_mark(efirst, mark)) {
                break;
            }
            if let Some(right) = self.half(e).rightv {
                match mark {
                    EdgeMark::Seam => self.bound_mut(right).seam_len = count,
                    EdgeMark::Sharp => self.bound_mut(right).sharp_len = count,
                }
            }
            e = ne;
            if e == efirst {
                break;
            }
        }
    }

    pub(crate) fn extend_edge_data(&mut self, bev: BevId) {
        if self.vmesh(bev).kind == MeshKind::TriFan || self.bev(bev).selcount < 2 {
            return;
        }
        self.extend_mark(bev, EdgeMark::Seam);
        self.extend_mark(bev, EdgeMark::Sharp);
    }

    fn mark_between(
        &mut self,
        a: Option<rikka_mesh_edit::VertId>,
        b: Option<rikka_mesh_edit::VertId>,
        mark: EdgeMark,
    ) {
        let (Some(a), Some(b)) = (a, b) else {
            return;
        };
        let Some(edge) = self.mesh.edge_between(a, b) else {
            return;
        };
        match mark {
            EdgeMark::Seam => self.mesh.set_edge_seam(edge, true),
            EdgeMark::Sharp => self.mesh.set_edge_sharp(edge, true),
        }
    }

    fn extend_mark(&mut self, bev: BevId, mark: EdgeMark) {
        let Some(boundstart) = self.vmesh(bev).boundstart else {
            return;
        };
        let count = self.vmesh(bev).count;
        let seg = self.vmesh(bev).seg;
        let limit = count * (count + 2);
        let mut steps = 0;
        let mut bcur = boundstart;
        let mut start = bcur;
        loop {
            steps += 1;
            let extend = match mark {
                EdgeMark::Seam => self.bound(bcur).seam_len,
                EdgeMark::Sharp => self.bound(bcur).sharp_len,
            };
            if extend > 0 {
                if self.bound(boundstart).seam_len == 0 && start == boundstart {
                    start = bcur;
                }
                let first = self.bound(bcur).index;
                for i in first..first + extend {
                    let grid = self.vmesh(bev);
                    let mut v1 = grid.vert(i % count, 0, 0);
                    let mut pairs = Vec::with_capacity(seg);
                    for k in 1..seg {
                        let v2 = grid.vert(i % count, 0, k);
                        pairs.push((v1, v2));
                        v1 = v2;
                    }
                    pairs.push((v1, grid.vert((i + 1) % count, 0, 0)));
                    for (a, b) in pairs {
                        self.mark_between(a, b, mark);
                    }
                    bcur = self.bound(bcur).next;
                }
            } else {
                bcur = self.bound(bcur).next;
            }
            if bcur == start || steps > limit {
                break;
            }
        }
    }
}
