use crate::ids::{EdgeId, FaceId};
use crate::mesh::EditMesh;
use crate::sets::IdSet;

impl EditMesh {
    #[must_use]
    pub fn calc_face_groups(&self, test: impl Fn(FaceId) -> bool) -> Vec<Vec<FaceId>> {
        let mut visited: IdSet<FaceId> = IdSet::with_capacity(self.face_capacity());
        let mut remaining = 0usize;
        for face in self.faces() {
            match test(face) {
                true => remaining += 1,
                false => {
                    visited.insert(face);
                }
            }
        }
        let mut groups = Vec::new();
        let mut stack: Vec<FaceId> = Vec::new();
        let mut seeds = self.faces();
        while remaining > 0 {
            let Some(seed) = seeds.by_ref().find(|&face| !visited.contains(face)) else {
                break;
            };
            visited.insert(seed);
            stack.push(seed);
            let mut group = Vec::new();
            while let Some(face) = stack.pop() {
                group.push(face);
                remaining -= 1;
                for lp in self.face_loops(face) {
                    let mut radial = self.loop_radial_next(lp);
                    while radial != lp {
                        let other = self.loop_face(radial);
                        if visited.insert(other) {
                            stack.push(other);
                        }
                        radial = self.loop_radial_next(radial);
                    }
                }
            }
            groups.push(group);
        }
        groups
    }

    #[must_use]
    pub fn calc_edge_groups(&self, test: impl Fn(EdgeId) -> bool) -> Vec<Vec<EdgeId>> {
        let mut visited: IdSet<EdgeId> = IdSet::with_capacity(self.edge_capacity());
        let mut remaining = 0usize;
        for edge in self.edges() {
            match test(edge) {
                true => remaining += 1,
                false => {
                    visited.insert(edge);
                }
            }
        }
        let mut groups = Vec::new();
        let mut stack: Vec<EdgeId> = Vec::new();
        let mut seeds = self.edges();
        while remaining > 0 {
            let Some(seed) = seeds.by_ref().find(|&edge| !visited.contains(edge)) else {
                break;
            };
            visited.insert(seed);
            stack.push(seed);
            let mut group = Vec::new();
            while let Some(edge) = stack.pop() {
                group.push(edge);
                remaining -= 1;
                for vert in self.edge_verts(edge) {
                    for other in self.vert_edges(vert) {
                        if visited.insert(other) {
                            stack.push(other);
                        }
                    }
                }
            }
            groups.push(group);
        }
        groups
    }
}
