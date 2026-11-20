use glam::Vec3;
use rikka_model_core::polygon;

use crate::ids::{FaceId, LoopId, VertId};
use crate::mesh::EditMesh;
use crate::parallel;
use crate::sets::IdSet;

const NO_START: u32 = u32::MAX;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tessellation {
    pub triangles: Vec<[LoopId; 3]>,
    pub faces: Vec<FaceId>,
    face_starts: Vec<u32>,
    source: (u64, u64),
}

impl Tessellation {
    #[must_use]
    pub fn face_triangles(&self, face: FaceId) -> &[[LoopId; 3]] {
        match self.face_starts.get(face.index()) {
            Some(&start) if start != NO_START => {
                let end = self.faces[start as usize..]
                    .iter()
                    .position(|&owner| owner != face)
                    .map_or(self.faces.len(), |offset| start as usize + offset);
                &self.triangles[start as usize..end]
            }
            _ => &[],
        }
    }

    #[must_use]
    pub fn is_current(&self, mesh: &EditMesh) -> bool {
        let revisions = mesh.revisions();
        self.source == (revisions.mesh, revisions.topology)
    }

    pub fn update(&mut self, mesh: &EditMesh, moved: impl IntoIterator<Item = VertId>) {
        if !self.is_current(mesh) {
            *self = mesh.tessellate();
            return;
        }
        let mut faces: IdSet<FaceId> = IdSet::new();
        for vert in moved {
            if mesh.contains_vert(vert) {
                faces.extend(mesh.vert_faces(vert));
            }
        }
        let mut scratch = Vec::new();
        for face in faces.iter() {
            let start = self.face_starts[face.index()] as usize;
            scratch.clear();
            mesh.face_triangles(face, false, &mut scratch);
            self.triangles[start..start + scratch.len()].copy_from_slice(&scratch);
        }
    }
}

impl EditMesh {
    #[must_use]
    pub fn tessellate(&self) -> Tessellation {
        let faces: Vec<FaceId> = self.faces().collect();
        let owned = parallel::concat(faces.len(), |range| {
            let mut triangles = Vec::new();
            let mut owned = Vec::new();
            for &face in &faces[range] {
                let start = triangles.len();
                self.face_triangles(face, false, &mut triangles);
                owned.extend(triangles[start..].iter().map(|&triangle| (triangle, face)));
            }
            owned
        });
        let mut tessellation = Tessellation {
            triangles: owned.iter().map(|&(triangle, _)| triangle).collect(),
            faces: owned.iter().map(|&(_, face)| face).collect(),
            face_starts: vec![NO_START; self.faces.capacity()],
            source: (self.stamp.0, self.counters.topology),
        };
        let mut cursor = 0;
        for face in faces {
            tessellation.face_starts[face.index()] = cursor as u32;
            while cursor < tessellation.faces.len() && tessellation.faces[cursor] == face {
                cursor += 1;
            }
        }
        tessellation
    }

    pub fn face_triangles(&self, face: FaceId, fixed_quad: bool, out: &mut Vec<[LoopId; 3]>) {
        let first = self.face_first_loop(face);
        match self.face_len(face) {
            0..=2 => {}
            3 => {
                let second = self.loop_next(first);
                out.push([first, second, self.loop_next(second)]);
            }
            4 => {
                let second = self.loop_next(first);
                let third = self.loop_next(second);
                let fourth = self.loop_next(third);
                let at = |lp: LoopId| self.position(self.loop_vert(lp));
                match !fixed_quad && quad_flips(at(first), at(second), at(third), at(fourth)) {
                    false => {
                        out.push([first, second, third]);
                        out.push([first, third, fourth]);
                    }
                    true => {
                        out.push([first, second, fourth]);
                        out.push([second, third, fourth]);
                    }
                }
            }
            _ => {
                let loops = self.face_loops_vec(face);
                let normal = self.calc_face_normal(face);
                let projected = polygon::project(
                    loops.iter().map(|&lp| self.position(self.loop_vert(lp))),
                    normal,
                );
                for triangle in polygon::triangulate_flat(&projected) {
                    out.push(triangle.map(|slot| loops[slot as usize]));
                }
            }
        }
    }
}

pub(crate) fn quad_flips(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let along_ab = b - a;
    let along_ac = c - a;
    let along_ad = d - a;
    along_ab.cross(along_ac).dot(along_ad.cross(along_ac)) > 0.0
}
