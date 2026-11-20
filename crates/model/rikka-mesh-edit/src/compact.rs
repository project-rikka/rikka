use crate::arena::Arena;
use crate::ids::{EdgeId, Element, FaceId, LoopId, NONE, VertId};
use crate::mesh::EditMesh;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Compaction {
    verts: Vec<u32>,
    edges: Vec<u32>,
    loops: Vec<u32>,
    faces: Vec<u32>,
}

fn lookup(map: &[u32], index: usize) -> Option<u32> {
    map.get(index).copied().filter(|&new| new != NONE)
}

impl Compaction {
    #[must_use]
    pub fn map_vert(&self, old: VertId) -> Option<VertId> {
        lookup(&self.verts, old.index()).map(VertId)
    }

    #[must_use]
    pub fn map_edge(&self, old: EdgeId) -> Option<EdgeId> {
        lookup(&self.edges, old.index()).map(EdgeId)
    }

    #[must_use]
    pub fn map_loop(&self, old: LoopId) -> Option<LoopId> {
        lookup(&self.loops, old.index()).map(LoopId)
    }

    #[must_use]
    pub fn map_face(&self, old: FaceId) -> Option<FaceId> {
        lookup(&self.faces, old.index()).map(FaceId)
    }
}

fn remap(map: &[u32], index: u32) -> u32 {
    match index {
        NONE => NONE,
        index => map[index as usize],
    }
}

impl EditMesh {
    pub fn compact(&mut self) -> Compaction {
        let verts: Vec<u32> = self.verts.indices().collect();
        let edges: Vec<u32> = self.edges.indices().collect();
        let faces: Vec<u32> = self.faces.indices().collect();
        let mut loops: Vec<u32> = Vec::with_capacity(self.loops.live());
        for &face in &faces {
            loops.extend(self.face_loops(FaceId(face)).map(|lp| lp.0));
        }
        let inverse = |order: &[u32], capacity: usize| {
            let mut map = vec![NONE; capacity];
            for (new, &old) in order.iter().enumerate() {
                map[old as usize] = new as u32;
            }
            map
        };
        let compaction = Compaction {
            verts: inverse(&verts, self.verts.capacity()),
            edges: inverse(&edges, self.edges.capacity()),
            loops: inverse(&loops, self.loops.capacity()),
            faces: inverse(&faces, self.faces.capacity()),
        };
        let map = &compaction;

        let vert_records = verts
            .iter()
            .map(|&old| {
                let mut record = self.verts.records[old as usize];
                record.edge = remap(&map.edges, record.edge);
                record
            })
            .collect();
        let edge_records = edges
            .iter()
            .map(|&old| {
                let mut record = self.edges.records[old as usize];
                record.verts = record.verts.map(|vert| remap(&map.verts, vert));
                for link in &mut record.disk {
                    link.prev = remap(&map.edges, link.prev);
                    link.next = remap(&map.edges, link.next);
                }
                record.first_loop = remap(&map.loops, record.first_loop);
                record
            })
            .collect();
        let loop_records = loops
            .iter()
            .map(|&old| {
                let mut record = self.loops.records[old as usize];
                record.vert = remap(&map.verts, record.vert);
                record.edge = remap(&map.edges, record.edge);
                record.face = remap(&map.faces, record.face);
                record.next = remap(&map.loops, record.next);
                record.prev = remap(&map.loops, record.prev);
                record.radial_next = remap(&map.loops, record.radial_next);
                record.radial_prev = remap(&map.loops, record.radial_prev);
                record
            })
            .collect();
        let face_records = faces
            .iter()
            .map(|&old| {
                let mut record = self.faces.records[old as usize];
                record.first = remap(&map.loops, record.first);
                record
            })
            .collect();

        self.positions = verts
            .iter()
            .map(|&old| self.positions[old as usize])
            .collect();
        self.vert_normals = verts
            .iter()
            .map(|&old| self.vert_normals[old as usize])
            .collect();
        self.face_normals = faces
            .iter()
            .map(|&old| self.face_normals[old as usize])
            .collect();
        self.vert_data.gather(&verts);
        self.edge_data.gather(&edges);
        self.loop_data.gather(&loops);
        self.face_data.gather(&faces);
        self.verts = Arena::rebuild(vert_records);
        self.edges = Arena::rebuild(edge_records);
        self.loops = Arena::rebuild(loop_records);
        self.faces = Arena::rebuild(face_records);
        for element in &mut self.history {
            *element = match *element {
                Element::Vert(vert) => Element::Vert(VertId(map.verts[vert.index()])),
                Element::Edge(edge) => Element::Edge(EdgeId(map.edges[edge.index()])),
                Element::Face(face) => Element::Face(FaceId(map.faces[face.index()])),
            };
        }
        self.active_face = self.active_face.map(|face| map.faces[face as usize]);
        self.touch_topology();
        compaction
    }
}
