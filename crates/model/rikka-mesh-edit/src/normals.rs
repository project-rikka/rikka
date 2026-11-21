use glam::Vec3;
use rikka_polymesh::acos_approx;

use crate::PARALLEL;
use crate::ids::{FaceId, NONE, VertId};
use crate::math::{newell_from, normal_quad, normal_tri, normalize};
use crate::mesh::EditMesh;
use crate::sets::IdSet;

const WHOLE_MESH_SHARE: usize = 4;

impl EditMesh {
    #[must_use]
    pub fn vert_normal(&self, vert: VertId) -> Vec3 {
        self.vert_normals[vert.index()]
    }

    #[must_use]
    pub fn face_normal(&self, face: FaceId) -> Vec3 {
        self.face_normals[face.index()]
    }

    pub fn set_vert_normal(&mut self, vert: VertId, normal: Vec3) {
        self.vert_normals[vert.index()] = normal;
    }

    pub fn set_face_normal(&mut self, face: FaceId, normal: Vec3) {
        self.face_normals[face.index()] = normal;
    }

    #[must_use]
    pub fn vert_normals(&self) -> &[Vec3] {
        &self.vert_normals
    }

    #[must_use]
    pub fn face_normals(&self) -> &[Vec3] {
        &self.face_normals
    }

    #[must_use]
    pub fn calc_face_normal(&self, face: FaceId) -> Vec3 {
        let first = self.faces.records[face.index()].first;
        let len = self.faces.records[face.index()].len;
        let position = |lp: u32| self.positions[self.loops.records[lp as usize].vert as usize];
        let next = |lp: u32| self.loops.records[lp as usize].next;
        match len {
            3 => normal_tri(
                position(first),
                position(next(first)),
                position(next(next(first))),
            ),
            4 => {
                let second = next(first);
                let third = next(second);
                normal_quad(
                    position(first),
                    position(second),
                    position(third),
                    position(next(third)),
                )
            }
            _ => normalize(self.face_newell(face)),
        }
    }

    pub(crate) fn face_newell(&self, face: FaceId) -> Vec3 {
        let first = self.faces.records[face.index()].first;
        let last = self.loops.records[first as usize].prev;
        let position = |lp: u32| self.positions[self.loops.records[lp as usize].vert as usize];
        newell_from(
            position(last),
            self.face_loops(face).map(|lp| position(lp.0)),
        )
    }

    pub fn update_face_normal(&mut self, face: FaceId) {
        self.face_normals[face.index()] = self.calc_face_normal(face);
    }

    #[must_use]
    pub fn calc_vert_normal(&self, vert: VertId) -> Vec3 {
        let mut normal = Vec3::ZERO;
        let first = self.verts.records[vert.index()].edge;
        if first != NONE {
            for edge in self.vert_edges(vert) {
                let record = self.edges.records[edge.index()];
                if record.first_loop == NONE {
                    continue;
                }
                let along = normalize(
                    self.positions[record.verts[0] as usize]
                        - self.positions[record.verts[1] as usize],
                );
                for lp in self.edge_loops(edge) {
                    let corner = self.loops.records[lp.index()];
                    if corner.vert != vert.0 {
                        continue;
                    }
                    let previous = self.loops.records[corner.prev as usize];
                    let previous_edge = self.edges.records[previous.edge as usize];
                    let before = normalize(
                        self.positions[previous_edge.verts[0] as usize]
                            - self.positions[previous_edge.verts[1] as usize],
                    );
                    let mut dot = before.dot(along);
                    if (previous_edge.verts[0] == previous.vert) ^ (record.verts[0] == corner.vert)
                    {
                        dot = -dot;
                    }
                    normal += self.face_normals[corner.face as usize] * acos_approx(-dot);
                }
            }
            let normal = normalize(normal);
            if normal != Vec3::ZERO {
                return normal;
            }
        }
        normalize(self.positions[vert.index()])
    }

    pub fn update_normals(&mut self) {
        let face_normals = PARALLEL.collect(self.faces.capacity(), |face| {
            match self.faces.is_alive(face as u32) {
                true => self.calc_face_normal(FaceId(face as u32)),
                false => Vec3::ZERO,
            }
        });
        self.face_normals = face_normals;
        let vert_normals = PARALLEL.collect(self.verts.capacity(), |vert| {
            match self.verts.is_alive(vert as u32) {
                true => self.calc_vert_normal(VertId(vert as u32)),
                false => Vec3::ZERO,
            }
        });
        self.vert_normals = vert_normals;
    }

    pub fn update_normals_around(&mut self, moved: impl IntoIterator<Item = VertId>) {
        let moved: Vec<VertId> = moved.into_iter().collect();
        if moved.len() * WHOLE_MESH_SHARE >= self.verts.live() {
            self.update_normals();
            return;
        }
        let mut faces: IdSet<FaceId> = IdSet::new();
        let mut verts: IdSet<VertId> = IdSet::new();
        for vert in moved {
            if !self.contains_vert(vert) {
                continue;
            }
            verts.insert(vert);
            for face in self.vert_faces(vert) {
                faces.insert(face);
            }
        }
        let faces: Vec<FaceId> = faces.iter().collect();
        let face_normals =
            PARALLEL.collect(faces.len(), |index| self.calc_face_normal(faces[index]));
        for (&face, normal) in faces.iter().zip(face_normals) {
            self.face_normals[face.index()] = normal;
            for lp in self.face_loops(face) {
                verts.insert(self.loop_vert(lp));
            }
        }
        let verts: Vec<VertId> = verts.iter().collect();
        let vert_normals =
            PARALLEL.collect(verts.len(), |index| self.calc_vert_normal(verts[index]));
        for (&vert, normal) in verts.iter().zip(vert_normals) {
            self.vert_normals[vert.index()] = normal;
        }
    }
}
