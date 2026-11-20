use std::collections::VecDeque;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::cdt::EdgeOrigin;
use crate::cdt::kernel::Kernel;
use crate::cdt::triangulation::{GHOST, NONE, Triangulation, edge_key};

pub(crate) type FaceSet = SmallVec<[u32; 2]>;

impl<K: Kernel> Triangulation<K> {
    pub(crate) fn real_triangles(&self) -> Vec<u32> {
        (0..self.tris.len() as u32)
            .filter(|&tri| {
                let record = self.tris[tri as usize];
                record.is_alive() && !record.is_ghost()
            })
            .collect()
    }

    fn edge_origins(&self, a: u32, b: u32) -> &[crate::cdt::triangulation::Origin] {
        match a == GHOST || b == GHOST {
            true => &[],
            false => self
                .constraints
                .get(&edge_key(a, b))
                .map_or(&[], |info| info.origins.as_slice()),
        }
    }

    fn face_edge_seeds(&self) -> FxHashMap<u32, Vec<(u32, u32)>> {
        let mut seeds: FxHashMap<u32, Vec<(u32, u32)>> = FxHashMap::default();
        let mut keys: Vec<&(u32, u32)> = self.constraints.keys().collect();
        keys.sort_unstable();
        for key in keys {
            for origin in &self.constraints[key].origins {
                if let EdgeOrigin::FaceEdge { face, .. } = origin.kind {
                    let directed = match origin.forward {
                        true => (key.0, key.1),
                        false => (key.1, key.0),
                    };
                    seeds.entry(face).or_default().push(directed);
                }
            }
        }
        seeds
    }

    fn bounded_by_face(&self, a: u32, b: u32, face: u32) -> bool {
        self.edge_origins(a, b)
            .iter()
            .any(|origin| matches!(origin.kind, EdgeOrigin::FaceEdge { face: f, .. } if f == face))
    }

    fn flood_face(&self, face: u32, starts: &[u32], mark: &mut FxHashMap<u32, ()>) -> bool {
        let mut stack: Vec<u32> = starts.to_vec();
        mark.clear();
        while let Some(tri) = stack.pop() {
            if mark.contains_key(&tri) {
                continue;
            }
            let record = self.tris[tri as usize];
            if record.is_ghost() {
                return false;
            }
            mark.insert(tri, ());
            for i in 0..3 {
                let (a, b) = (record.v[(i + 1) % 3], record.v[(i + 2) % 3]);
                let neighbor = record.n[i];
                if neighbor == NONE
                    || self.bounded_by_face(a, b, face)
                    || mark.contains_key(&neighbor)
                {
                    continue;
                }
                stack.push(neighbor);
            }
        }
        true
    }

    pub(crate) fn face_membership(&self) -> FxHashMap<u32, FaceSet> {
        let mut membership: FxHashMap<u32, FaceSet> = FxHashMap::default();
        let seeds = self.face_edge_seeds();
        let mut faces: Vec<&u32> = seeds.keys().collect();
        faces.sort_unstable();
        let mut mark = FxHashMap::default();
        for &face in faces {
            let edges = &seeds[&face];
            let side = |reverse: bool| -> Vec<u32> {
                edges
                    .iter()
                    .filter_map(|&(a, b)| {
                        let (x, y) = match reverse {
                            true => (b, a),
                            false => (a, b),
                        };
                        self.find_edge(x, y).map(|(tri, _)| tri)
                    })
                    .collect()
            };
            let mut found = self.flood_face(face, &side(false), &mut mark);
            if !found {
                found = self.flood_face(face, &side(true), &mut mark);
            }
            if !found {
                continue;
            }
            let mut members: Vec<u32> = mark.keys().copied().collect();
            members.sort_unstable();
            for tri in members {
                membership.entry(tri).or_default().push(face);
            }
        }
        membership
    }

    pub(crate) fn outside_reachable(&self) -> Vec<bool> {
        let mut reached = vec![false; self.tris.len()];
        let mut stack: Vec<u32> = (0..self.tris.len() as u32)
            .filter(|&tri| {
                let record = self.tris[tri as usize];
                record.is_alive() && record.is_ghost()
            })
            .collect();
        while let Some(tri) = stack.pop() {
            if reached[tri as usize] {
                continue;
            }
            reached[tri as usize] = true;
            let record = self.tris[tri as usize];
            for i in 0..3 {
                let (a, b) = (record.v[(i + 1) % 3], record.v[(i + 2) % 3]);
                let neighbor = record.n[i];
                if neighbor != NONE && !reached[neighbor as usize] && !self.is_constrained(a, b) {
                    stack.push(neighbor);
                }
            }
        }
        reached
    }

    pub(crate) fn crossing_values(&self, nonzero: bool) -> Vec<i64> {
        let mut values = vec![i64::MIN; self.tris.len()];
        let mut queue: VecDeque<u32> = VecDeque::new();
        for tri in 0..self.tris.len() as u32 {
            let record = self.tris[tri as usize];
            if record.is_alive() && record.is_ghost() {
                values[tri as usize] = 0;
                queue.push_back(tri);
            }
        }
        while let Some(tri) = queue.pop_front() {
            let record = self.tris[tri as usize];
            let value = values[tri as usize];
            for i in 0..3 {
                let neighbor = record.n[i];
                if neighbor == NONE || values[neighbor as usize] != i64::MIN {
                    continue;
                }
                let (a, b) = (record.v[(i + 1) % 3], record.v[(i + 2) % 3]);
                let mut delta = 0i64;
                for origin in self.edge_origins(a, b) {
                    match nonzero {
                        false => delta += 1,
                        true => {
                            if matches!(origin.kind, EdgeOrigin::FaceEdge { .. }) {
                                let directed_ab = origin.forward == (a < b);
                                delta += match directed_ab {
                                    true => -1,
                                    false => 1,
                                };
                            }
                        }
                    }
                }
                values[neighbor as usize] = value + delta;
                queue.push_back(neighbor);
            }
        }
        values
    }
}
