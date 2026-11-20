use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::arrangement::Triangle;

pub(crate) type Edge = (u32, u32);

pub(crate) fn edge(a: u32, b: u32) -> Edge {
    (a.min(b), a.max(b))
}

pub(crate) struct Topology {
    edge_tris: FxHashMap<Edge, SmallVec<[u32; 2]>>,
    vert_tris: FxHashMap<u32, SmallVec<[u32; 8]>>,
}

impl Topology {
    pub(crate) fn new(triangles: &[Triangle]) -> Topology {
        let mut edge_tris: FxHashMap<Edge, SmallVec<[u32; 2]>> = FxHashMap::default();
        let mut vert_tris: FxHashMap<u32, SmallVec<[u32; 8]>> = FxHashMap::default();
        edge_tris.reserve(triangles.len() * 2);
        for (index, triangle) in triangles.iter().enumerate() {
            let index = index as u32;
            for i in 0..3 {
                let key = edge(triangle.verts[i], triangle.verts[(i + 1) % 3]);
                let list = edge_tris.entry(key).or_default();
                if !list.contains(&index) {
                    list.push(index);
                }
                vert_tris.entry(triangle.verts[i]).or_default().push(index);
            }
        }
        Topology {
            edge_tris,
            vert_tris,
        }
    }

    pub(crate) fn edge_tris(&self, key: Edge) -> &[u32] {
        self.edge_tris.get(&key).map_or(&[], |list| list.as_slice())
    }

    pub(crate) fn other_if_manifold(&self, key: Edge, triangle: u32) -> Option<u32> {
        let list = self.edge_tris(key);
        match list.len() == 2 {
            true => Some(if list[0] == triangle {
                list[1]
            } else {
                list[0]
            }),
            false => None,
        }
    }

    pub(crate) fn vert_tris(&self, vertex: u32) -> &[u32] {
        self.vert_tris
            .get(&vertex)
            .map_or(&[], |list| list.as_slice())
    }

    pub(crate) fn vert_edges(
        &self,
        triangles: &[Triangle],
        vertex: u32,
        allowed: impl Fn(u32) -> bool,
    ) -> Vec<Edge> {
        let mut edges: Vec<Edge> = Vec::new();
        for &tri in self.vert_tris(vertex) {
            if !allowed(tri) {
                continue;
            }
            let verts = triangles[tri as usize].verts;
            let position = verts.iter().position(|&v| v == vertex).unwrap_or(0);
            for other in [verts[(position + 1) % 3], verts[(position + 2) % 3]] {
                let key = edge(vertex, other);
                if !edges.contains(&key) {
                    edges.push(key);
                }
            }
        }
        edges
    }

    pub(crate) fn is_pwn(&self, triangles: &[Triangle]) -> bool {
        self.edge_tris.iter().all(|(&(low, high), list)| {
            let total: i32 = list
                .iter()
                .map(|&tri| {
                    let verts = triangles[tri as usize].verts;
                    let position = verts.iter().position(|&v| v == low).unwrap_or(0);
                    match verts[(position + 1) % 3] == high {
                        true => 1,
                        false => -1,
                    }
                })
                .sum();
            total == 0
        })
    }
}
