use crate::ids::{EdgeId, FaceId, LoopId};
use crate::mesh::EditMesh;
use crate::select_ops::LinkedDelimit;
use crate::sets::IdSet;
use crate::walk::Walk;

impl EditMesh {
    #[must_use]
    pub fn loop_uv_shares_vert(&self, a: LoopId, b: LoopId, uv_map: &str) -> bool {
        let Some(uvs) = self.loop_uvs(uv_map) else {
            return false;
        };
        self.loop_vert(a) == self.loop_vert(b) && uvs[a.index()] == uvs[b.index()]
    }

    #[must_use]
    pub fn loop_uv_shares_edge(&self, a: LoopId, b: LoopId, uv_map: &str) -> bool {
        let Some(uvs) = self.loop_uvs(uv_map) else {
            return false;
        };
        if self.loop_edge(a) != self.loop_edge(b) {
            return false;
        }
        let (b_start, b_end) = match self.loop_vert(a) == self.loop_vert(b) {
            true => (b, self.loop_next(b)),
            false => (self.loop_next(b), b),
        };
        uvs[a.index()] == uvs[b_start.index()]
            && uvs[self.loop_next(a).index()] == uvs[b_end.index()]
    }

    fn walkable_edges(&self, delimit: LinkedDelimit) -> IdSet<EdgeId> {
        let uv_map = self.uv_maps().active_name().map(str::to_owned);
        self.edges()
            .filter(|&edge| !self.edge_delimits(edge, delimit, uv_map.as_deref()))
            .collect()
    }

    #[must_use]
    pub fn walk_island_delimited(&self, start: FaceId, delimit: LinkedDelimit) -> Vec<FaceId> {
        let walkable = self.walkable_edges(delimit);
        let walk = Walk {
            skip_hidden: true,
            edges: Some(&walkable),
            ..Walk::default()
        };
        self.walk_island(start, &walk, false)
    }

    #[must_use]
    pub fn face_islands(&self, delimit: LinkedDelimit) -> Vec<Vec<FaceId>> {
        let walkable = self.walkable_edges(delimit);
        let walk = Walk {
            skip_hidden: true,
            edges: Some(&walkable),
            ..Walk::default()
        };
        let mut seen: IdSet<FaceId> = IdSet::new();
        let mut islands = Vec::new();
        for face in self.faces() {
            if seen.contains(face) || self.is_face_hidden(face) {
                continue;
            }
            let island = self.walk_island(face, &walk, false);
            seen.extend(island.iter().copied());
            islands.push(island);
        }
        islands
    }
}
