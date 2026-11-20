use glam::{Vec2, Vec3};
use rikka_spatial::mean_value_weights_2d;

use crate::data::DataTable;
use crate::ids::{EdgeId, FaceId, LoopId, VertId};
use crate::math::{AxisProjection, normalize, ortho_vector};
use crate::mesh::EditMesh;

#[derive(Clone, Debug)]
pub struct FaceSample {
    positions: Vec<Vec3>,
    normal: Vec3,
    loops: DataTable,
    verts: DataTable,
}

impl FaceSample {
    #[must_use]
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    #[must_use]
    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    #[must_use]
    pub fn weights(&self, point: Vec3) -> Vec<(u32, f32)> {
        weights_in_polygon(&self.positions, self.normal, point)
    }
}

pub(crate) fn weights_in_polygon(polygon: &[Vec3], normal: Vec3, point: Vec3) -> Vec<(u32, f32)> {
    let projection = AxisProjection::new(normal);
    let coords: Vec<Vec2> = polygon
        .iter()
        .map(|&corner| projection.project(corner))
        .collect();
    let mut weights = vec![0.0; polygon.len()];
    mean_value_weights_2d(&coords, projection.project(point), &mut weights);
    weights
        .into_iter()
        .enumerate()
        .map(|(index, weight)| (index as u32, weight))
        .collect()
}

fn ids<I: Copy>(sources: &[(I, f32)], index: impl Fn(I) -> u32) -> Vec<(u32, f32)> {
    sources
        .iter()
        .map(|&(id, weight)| (index(id), weight))
        .collect()
}

impl EditMesh {
    pub fn copy_vert_data(&mut self, from: VertId, to: VertId) {
        self.copy_vert_attrs(from.0, to.0);
        self.touch_data();
    }

    pub fn copy_edge_data(&mut self, from: EdgeId, to: EdgeId) {
        self.copy_edge_attrs(from.0, to.0);
        self.touch_data();
    }

    pub fn copy_face_data(&mut self, from: FaceId, to: FaceId) {
        self.copy_face_attrs(from.0, to.0);
        self.touch_data();
    }

    pub fn copy_loop_data(&mut self, from: LoopId, to: LoopId) {
        self.copy_loop_attrs(from.0, to.0);
        self.touch_data();
    }

    pub fn interpolate_vert_data(&mut self, sources: &[(VertId, f32)], to: VertId) {
        self.vert_data.mix(&ids(sources, |vert| vert.0), to.index());
        self.touch_data();
    }

    pub fn interpolate_edge_data(&mut self, sources: &[(EdgeId, f32)], to: EdgeId) {
        self.edge_data.mix(&ids(sources, |edge| edge.0), to.index());
        self.touch_data();
    }

    pub fn interpolate_loop_data(&mut self, sources: &[(LoopId, f32)], to: LoopId) {
        self.loop_data.mix(&ids(sources, |lp| lp.0), to.index());
        self.touch_data();
    }

    pub fn interpolate_face_data(&mut self, sources: &[(FaceId, f32)], to: FaceId) {
        self.face_data.mix(&ids(sources, |face| face.0), to.index());
        self.touch_data();
    }

    pub fn average_vert_data(&mut self, sources: &[VertId], to: VertId) {
        let weight = 1.0 / sources.len().max(1) as f32;
        let sources: Vec<(VertId, f32)> = sources.iter().map(|&vert| (vert, weight)).collect();
        self.interpolate_vert_data(&sources, to);
    }

    pub fn average_edge_data(&mut self, sources: &[EdgeId], to: EdgeId) {
        let weight = 1.0 / sources.len().max(1) as f32;
        let sources: Vec<(EdgeId, f32)> = sources.iter().map(|&edge| (edge, weight)).collect();
        self.interpolate_edge_data(&sources, to);
    }

    pub fn average_loop_data(&mut self, sources: &[LoopId], to: LoopId) {
        let weight = 1.0 / sources.len().max(1) as f32;
        let sources: Vec<(LoopId, f32)> = sources.iter().map(|&lp| (lp, weight)).collect();
        self.interpolate_loop_data(&sources, to);
    }

    pub fn average_face_data(&mut self, sources: &[FaceId], to: FaceId) {
        let weight = 1.0 / sources.len().max(1) as f32;
        let sources: Vec<(FaceId, f32)> = sources.iter().map(|&face| (face, weight)).collect();
        self.interpolate_face_data(&sources, to);
    }

    pub fn interpolate_vert_data_between(&mut self, a: VertId, b: VertId, to: VertId, factor: f32) {
        self.mix_pair(crate::Domain::Point, a.0, b.0, to.0, factor);
    }

    pub fn interpolate_edge_data_between(&mut self, a: EdgeId, b: EdgeId, to: EdgeId, factor: f32) {
        self.mix_pair(crate::Domain::Edge, a.0, b.0, to.0, factor);
    }

    pub fn interpolate_loop_data_between(&mut self, a: LoopId, b: LoopId, to: LoopId, factor: f32) {
        self.mix_pair(crate::Domain::Corner, a.0, b.0, to.0, factor);
    }

    pub(crate) fn mix_pair(&mut self, domain: crate::Domain, a: u32, b: u32, to: u32, factor: f32) {
        let table = self.table_mut(domain);
        if factor <= 0.0 {
            table.copy(a as usize, to as usize);
        } else if factor >= 1.0 {
            table.copy(b as usize, to as usize);
        } else {
            table.mix(&[(a, 1.0 - factor), (b, factor)], to as usize);
        }
        self.touch_data();
    }

    pub fn interpolate_face_vert_edge(
        &mut self,
        start: VertId,
        vert: VertId,
        edge: EdgeId,
        factor: f32,
    ) {
        let loops: Vec<LoopId> = self.edge_loops(edge).collect();
        for lp in loops {
            let (first, middle, last) = if self.loop_vert(lp) == start {
                let middle = self.loop_next(lp);
                (lp, middle, self.loop_next(middle))
            } else if self.loop_vert(lp) == vert {
                (self.loop_next(lp), lp, self.loop_prev(lp))
            } else {
                return;
            };
            self.loop_data
                .mix(&[(first.0, factor), (last.0, 1.0 - factor)], middle.index());
        }
        self.touch_data();
    }

    pub(crate) fn interpolation_normal(&self, face: FaceId) -> Vec3 {
        let normal = self.calc_face_normal(face);
        if normal != Vec3::ZERO {
            return normal;
        }
        normalize(ortho_vector(self.calc_face_tangent_auto(face)))
    }

    #[must_use]
    pub fn sample_face(&self, face: FaceId) -> FaceSample {
        let loops: Vec<u32> = self.face_loops(face).map(|lp| lp.0).collect();
        let verts: Vec<u32> = loops
            .iter()
            .map(|&lp| self.loops.records[lp as usize].vert)
            .collect();
        FaceSample {
            positions: verts
                .iter()
                .map(|&vert| self.positions[vert as usize])
                .collect(),
            normal: self.interpolation_normal(face),
            loops: self.loop_data.extract(&loops),
            verts: self.vert_data.extract(&verts),
        }
    }

    pub fn interpolate_loop_from_sample(
        &mut self,
        lp: LoopId,
        sample: &FaceSample,
        with_vert: bool,
    ) {
        let vert = self.loop_vert(lp);
        let weights = sample.weights(self.positions[vert.index()]);
        self.loop_data.mix_from(&sample.loops, &weights, lp.index());
        if with_vert {
            self.vert_data
                .mix_from(&sample.verts, &weights, vert.index());
        }
        self.touch_data();
    }

    pub fn interpolate_vert_from_sample(&mut self, vert: VertId, sample: &FaceSample) {
        let weights = sample.weights(self.positions[vert.index()]);
        self.vert_data
            .mix_from(&sample.verts, &weights, vert.index());
        self.touch_data();
    }

    pub fn interpolate_face_from_sample(
        &mut self,
        face: FaceId,
        sample: &FaceSample,
        with_verts: bool,
    ) {
        let loops = self.face_loops_vec(face);
        for lp in loops {
            self.interpolate_loop_from_sample(lp, sample, with_verts);
        }
    }

    pub fn interpolate_loop_from_face(&mut self, lp: LoopId, source: FaceId, with_vert: bool) {
        let sample = self.sample_face(source);
        self.interpolate_loop_from_sample(lp, &sample, with_vert);
    }

    pub fn interpolate_vert_from_face(&mut self, vert: VertId, source: FaceId) {
        let sample = self.sample_face(source);
        self.interpolate_vert_from_sample(vert, &sample);
    }

    pub fn interpolate_face_from_face(&mut self, face: FaceId, source: FaceId, with_verts: bool) {
        let sample = self.sample_face(source);
        self.interpolate_face_from_sample(face, &sample, with_verts);
    }

    pub fn copy_face_loop_data_shared(&mut self, face: FaceId) {
        self.copy_face_loop_data_shared_where(face, |_, _| true);
    }

    pub fn copy_face_loop_data_shared_where(
        &mut self,
        face: FaceId,
        filter: impl Fn(&EditMesh, LoopId) -> bool,
    ) {
        let loops = self.face_loops_vec(face);
        let mut done = vec![false; loops.len()];
        for (index, &lp) in loops.iter().enumerate() {
            let radial = self.loop_radial_next(lp);
            if radial == lp {
                continue;
            }
            let (first, second) = match self.loop_vert(radial) == self.loop_vert(lp) {
                true => (radial, self.loop_next(radial)),
                false => (self.loop_next(radial), radial),
            };
            let next_index = (index + 1) % loops.len();
            for (slot, source) in [(index, first), (next_index, second)] {
                if !done[slot] && filter(self, source) {
                    self.loop_data.copy(source.index(), loops[slot].index());
                    done[slot] = true;
                }
            }
        }
        self.touch_data();
    }
}
