use glam::Vec3;
use rikka_mesh_edit::{EdgeId, FaceId, VertId};

use crate::spacing::PRO_LINE_R;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct HalfId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BoundId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct BevId(pub(crate) u32);

#[derive(Clone, Debug)]
pub(crate) struct EdgeHalf {
    pub(crate) owner: BevId,
    pub(crate) edge: EdgeId,
    pub(crate) fprev: Option<FaceId>,
    pub(crate) fnext: Option<FaceId>,
    pub(crate) leftv: Option<BoundId>,
    pub(crate) rightv: Option<BoundId>,
    pub(crate) profile_index: usize,
    pub(crate) seg: usize,
    pub(crate) offset_l: f32,
    pub(crate) offset_r: f32,
    pub(crate) offset_l_spec: f32,
    pub(crate) offset_r_spec: f32,
    pub(crate) is_bev: bool,
    pub(crate) is_rev: bool,
    pub(crate) is_seam: bool,
    pub(crate) visited_rpo: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Profile {
    pub(crate) super_r: f32,
    pub(crate) height: f32,
    pub(crate) start: Vec3,
    pub(crate) middle: Vec3,
    pub(crate) end: Vec3,
    pub(crate) plane_no: Vec3,
    pub(crate) plane_co: Vec3,
    pub(crate) proj_dir: Vec3,
    pub(crate) points: Vec<Vec3>,
    pub(crate) power_points: Option<Vec<Vec3>>,
    pub(crate) special_params: bool,
}

impl Profile {
    pub(crate) fn power_points(&self) -> &[Vec3] {
        self.power_points.as_deref().unwrap_or(&self.points)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BoundVert {
    pub(crate) next: BoundId,
    pub(crate) prev: BoundId,
    pub(crate) co: Vec3,
    pub(crate) vert: Option<VertId>,
    pub(crate) efirst: Option<HalfId>,
    pub(crate) elast: Option<HalfId>,
    pub(crate) eon: Option<HalfId>,
    pub(crate) ebev: Option<HalfId>,
    pub(crate) index: usize,
    pub(crate) sinratio: f32,
    pub(crate) adjchain: Option<BoundId>,
    pub(crate) profile: Profile,
    pub(crate) any_seam: bool,
    pub(crate) visited: bool,
    pub(crate) is_arc_start: bool,
    pub(crate) is_patch_start: bool,
    pub(crate) is_profile_start: bool,
    pub(crate) seam_len: usize,
    pub(crate) sharp_len: usize,
}

impl BoundVert {
    pub(crate) fn new(co: Vec3, id: BoundId, index: usize) -> BoundVert {
        BoundVert {
            next: id,
            prev: id,
            co,
            vert: None,
            efirst: None,
            elast: None,
            eon: None,
            ebev: None,
            index,
            sinratio: 1.0,
            adjchain: None,
            profile: Profile {
                super_r: PRO_LINE_R,
                ..Profile::default()
            },
            any_seam: false,
            visited: false,
            is_arc_start: false,
            is_patch_start: false,
            is_profile_start: false,
            seam_len: 0,
            sharp_len: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MeshKind {
    #[default]
    None,
    Poly,
    Adj,
    TriFan,
    Cutoff,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NewVert {
    pub(crate) vert: Option<VertId>,
    pub(crate) co: Vec3,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Grid {
    pub(crate) cells: Vec<NewVert>,
    pub(crate) boundstart: Option<BoundId>,
    pub(crate) count: usize,
    pub(crate) seg: usize,
    pub(crate) kind: MeshKind,
}

impl Grid {
    pub(crate) fn new(count: usize, seg: usize, boundstart: Option<BoundId>) -> Grid {
        let mut grid = Grid {
            cells: Vec::new(),
            boundstart,
            count,
            seg,
            kind: MeshKind::Adj,
        };
        grid.allocate();
        grid
    }

    pub(crate) fn allocate(&mut self) {
        self.cells = vec![NewVert::default(); self.count * (self.seg / 2 + 1) * (self.seg + 1)];
    }

    fn slot(&self, i: usize, j: usize, k: usize) -> usize {
        let nj = self.seg / 2 + 1;
        let nk = self.seg + 1;
        i * nk * nj + j * nk + k
    }

    pub(crate) fn at(&self, i: usize, j: usize, k: usize) -> &NewVert {
        &self.cells[self.slot(i, j, k)]
    }

    pub(crate) fn at_mut(&mut self, i: usize, j: usize, k: usize) -> &mut NewVert {
        let slot = self.slot(i, j, k);
        &mut self.cells[slot]
    }

    pub(crate) fn co(&self, i: usize, j: usize, k: usize) -> Vec3 {
        self.at(i, j, k).co
    }

    pub(crate) fn vert(&self, i: usize, j: usize, k: usize) -> Option<VertId> {
        self.at(i, j, k).vert
    }

    pub(crate) fn set_co(&mut self, i: usize, j: usize, k: usize, co: Vec3) {
        self.at_mut(i, j, k).co = co;
    }

    pub(crate) fn copy(&mut self, to: [usize; 3], from: [usize; 3]) {
        let source = *self.at(from[0], from[1], from[2]);
        *self.at_mut(to[0], to[1], to[2]) = source;
    }

    pub(crate) fn canon_index(&self, i: usize, j: usize, k: usize) -> [usize; 3] {
        let n = self.count;
        let ns = self.seg;
        let ns2 = ns / 2;
        let odd = ns % 2;
        if odd == 0 && j == ns2 && k == ns2 {
            return [0, j, k];
        }
        if j < ns2 + odd && k <= ns2 {
            return [i, j, k];
        }
        if k <= ns2 {
            return [(i + n - 1) % n, k, ns - j];
        }
        [(i + 1) % n, ns - k, j]
    }

    pub(crate) fn canon(&self, i: usize, j: usize, k: usize) -> &NewVert {
        let [ci, cj, ck] = self.canon_index(i, j, k);
        self.at(ci, cj, ck)
    }

    pub(crate) fn canon_co(&self, i: usize, j: usize, k: usize) -> Vec3 {
        self.canon(i, j, k).co
    }

    pub(crate) fn set_canon_co(&mut self, i: usize, j: usize, k: usize, co: Vec3) {
        let [ci, cj, ck] = self.canon_index(i, j, k);
        self.set_co(ci, cj, ck, co);
    }

    pub(crate) fn is_canon(&self, i: usize, j: usize, k: usize) -> bool {
        let ns2 = self.seg / 2;
        match self.seg % 2 == 1 {
            true => j <= ns2 && k <= ns2,
            false => (j < ns2 && k <= ns2) || (j == ns2 && k == ns2 && i == 0),
        }
    }

    pub(crate) fn copy_equivalent(&mut self) {
        let ns2 = self.seg / 2;
        for i in 0..self.count {
            for j in 0..=ns2 {
                for k in 0..=self.seg {
                    if self.is_canon(i, j, k) {
                        continue;
                    }
                    let source = *self.canon(i, j, k);
                    *self.at_mut(i, j, k) = source;
                }
            }
        }
    }

    pub(crate) fn center(&self) -> Vec3 {
        let ns2 = self.seg / 2;
        if self.seg % 2 == 1 {
            let mut sum = Vec3::ZERO;
            for i in 0..self.count {
                sum += self.co(i, ns2, ns2);
            }
            sum * (1.0 / self.count as f32)
        } else {
            self.co(0, ns2, ns2)
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BevVert {
    pub(crate) vert: VertId,
    pub(crate) edgecount: usize,
    pub(crate) selcount: usize,
    pub(crate) offset: f32,
    pub(crate) any_seam: bool,
    pub(crate) first_half: u32,
    pub(crate) wire_edges: Vec<EdgeId>,
    pub(crate) vmesh: Grid,
}

impl BevVert {
    pub(crate) fn half(&self, local: usize) -> HalfId {
        HalfId(self.first_half + local as u32)
    }

    pub(crate) fn halves(&self) -> impl Iterator<Item = HalfId> + use<> {
        let first = self.first_half;
        (0..self.edgecount as u32).map(move |local| HalfId(first + local))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FaceKind {
    None,
    Orig,
    Vert,
    Edge,
    Recon,
}
