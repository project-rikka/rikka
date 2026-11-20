use glam::Vec3;
use rikka_mesh_edit::{EdgeId, EditMesh, FaceId, FaceSample, IdMap, IdSet, LoopId, VertId};

use crate::model::{BevId, BevVert, BoundId, BoundVert, EdgeHalf, FaceKind, Grid, HalfId};
use crate::params::Params;
use crate::uv::UvState;

pub(crate) struct Interpolation {
    pub(crate) sample: FaceSample,
    pub(crate) loops: Vec<LoopId>,
    pub(crate) verts: Vec<VertId>,
}

pub(crate) struct Bevel<'m> {
    pub(crate) mesh: &'m mut EditMesh,
    pub(crate) params: Params,
    pub(crate) halves: Vec<EdgeHalf>,
    pub(crate) bounds: Vec<BoundVert>,
    pub(crate) bev_verts: Vec<BevVert>,
    pub(crate) vert_bev: IdMap<VertId, BevId>,
    pub(crate) face_kinds: IdMap<FaceId, FaceKind>,
    pub(crate) uv: UvState,
    pub(crate) interpolations: IdMap<FaceId, Interpolation>,
    pub(crate) out_verts: IdSet<VertId>,
    pub(crate) out_edges: IdSet<EdgeId>,
    pub(crate) long_loops: IdSet<LoopId>,
}

impl<'m> Bevel<'m> {
    pub(crate) fn new(mesh: &'m mut EditMesh, params: Params) -> Bevel<'m> {
        Bevel {
            mesh,
            params,
            halves: Vec::new(),
            bounds: Vec::new(),
            bev_verts: Vec::new(),
            vert_bev: IdMap::new(),
            face_kinds: IdMap::new(),
            uv: UvState::default(),
            interpolations: IdMap::new(),
            out_verts: IdSet::new(),
            out_edges: IdSet::new(),
            long_loops: IdSet::new(),
        }
    }

    pub(crate) fn half(&self, half: HalfId) -> &EdgeHalf {
        &self.halves[half.0 as usize]
    }

    pub(crate) fn half_mut(&mut self, half: HalfId) -> &mut EdgeHalf {
        &mut self.halves[half.0 as usize]
    }

    pub(crate) fn bound(&self, bound: BoundId) -> &BoundVert {
        &self.bounds[bound.0 as usize]
    }

    pub(crate) fn bound_mut(&mut self, bound: BoundId) -> &mut BoundVert {
        &mut self.bounds[bound.0 as usize]
    }

    pub(crate) fn bev(&self, bev: BevId) -> &BevVert {
        &self.bev_verts[bev.0 as usize]
    }

    pub(crate) fn bev_mut(&mut self, bev: BevId) -> &mut BevVert {
        &mut self.bev_verts[bev.0 as usize]
    }

    pub(crate) fn vmesh(&self, bev: BevId) -> &Grid {
        &self.bev(bev).vmesh
    }

    pub(crate) fn vmesh_mut(&mut self, bev: BevId) -> &mut Grid {
        &mut self.bev_mut(bev).vmesh
    }

    pub(crate) fn bev_of(&self, vert: VertId) -> Option<BevId> {
        self.vert_bev.copied(vert)
    }

    fn local(&self, half: HalfId) -> (u32, u32) {
        let bev = self.bev(self.half(half).owner);
        (bev.first_half, bev.edgecount as u32)
    }

    pub(crate) fn next_half(&self, half: HalfId) -> HalfId {
        let (first, count) = self.local(half);
        HalfId(first + (half.0 - first + 1) % count)
    }

    pub(crate) fn prev_half(&self, half: HalfId) -> HalfId {
        let (first, count) = self.local(half);
        HalfId(first + (half.0 - first + count - 1) % count)
    }

    pub(crate) fn find_half(&self, bev: BevId, edge: EdgeId) -> Option<HalfId> {
        self.bev(bev)
            .halves()
            .find(|&half| self.half(half).edge == edge)
    }

    pub(crate) fn half_vert(&self, half: HalfId) -> VertId {
        self.bev(self.half(half).owner).vert
    }

    pub(crate) fn half_other_vert(&self, half: HalfId) -> VertId {
        let [a, b] = self.mesh.edge_verts(self.half(half).edge);
        match self.half(half).is_rev {
            true => a,
            false => b,
        }
    }

    pub(crate) fn other_end(&self, half: HalfId) -> Option<HalfId> {
        let other = self.half_other_vert(half);
        let bev = self.bev_of(other)?;
        self.find_half(bev, self.half(half).edge)
    }

    pub(crate) fn add_bound(&mut self, grid: &mut Grid, co: Vec3) -> BoundId {
        let id = BoundId(self.bounds.len() as u32);
        let index = match grid.boundstart {
            None => 0,
            Some(start) => self.bound(self.bound(start).prev).index + 1,
        };
        self.bounds.push(BoundVert::new(co, id, index));
        if let Some(start) = grid.boundstart {
            let tail = self.bound(start).prev;
            self.bound_mut(id).prev = tail;
            self.bound_mut(id).next = start;
            self.bound_mut(tail).next = id;
            self.bound_mut(start).prev = id;
        } else {
            grid.boundstart = Some(id);
        }
        grid.count += 1;
        id
    }

    pub(crate) fn add_bev_bound(&mut self, bev: BevId, co: Vec3) -> BoundId {
        let mut grid = std::mem::take(self.vmesh_mut(bev));
        let id = self.add_bound(&mut grid, co);
        *self.vmesh_mut(bev) = grid;
        id
    }

    pub(crate) fn ring(&self, start: BoundId) -> Vec<BoundId> {
        let mut ring = vec![start];
        let mut current = self.bound(start).next;
        while current != start {
            ring.push(current);
            current = self.bound(current).next;
        }
        ring
    }

    pub(crate) fn bev_ring(&self, bev: BevId) -> Vec<BoundId> {
        match self.vmesh(bev).boundstart {
            Some(start) => self.ring(start),
            None => Vec::new(),
        }
    }

    pub(crate) fn face_normal(&self, face: FaceId) -> Vec3 {
        self.mesh.face_normal(face)
    }

    pub(crate) fn position(&self, vert: VertId) -> Vec3 {
        self.mesh.position(vert)
    }

    pub(crate) fn record_face_kind(&mut self, face: FaceId, kind: FaceKind) {
        self.face_kinds.insert(face, kind);
    }

    pub(crate) fn face_kind(&self, face: FaceId) -> FaceKind {
        self.face_kinds.copied(face).unwrap_or(FaceKind::Orig)
    }
}
