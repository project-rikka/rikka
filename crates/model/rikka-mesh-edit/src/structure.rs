use glam::Vec3;

use crate::arena::Slot;
use crate::ids::{Element, NONE};
use crate::mesh::{DiskLink, EdgeRecord, EditMesh, FaceRecord, Flags, LoopRecord, VertRecord};

impl EditMesh {
    pub(crate) fn disk_side(&self, edge: u32, vert: u32) -> usize {
        let verts = self.edges.records[edge as usize].verts;
        debug_assert!(verts.contains(&vert), "vertex is not in the edge");
        usize::from(verts[0] != vert)
    }

    pub(crate) fn disk_next_raw(&self, edge: u32, vert: u32) -> u32 {
        let side = self.disk_side(edge, vert);
        self.edges.records[edge as usize].disk[side].next
    }

    pub(crate) fn disk_prev_raw(&self, edge: u32, vert: u32) -> u32 {
        let side = self.disk_side(edge, vert);
        self.edges.records[edge as usize].disk[side].prev
    }

    fn disk_link_mut(&mut self, edge: u32, vert: u32) -> &mut DiskLink {
        let side = self.disk_side(edge, vert);
        &mut self.edges.records[edge as usize].disk[side]
    }

    pub(crate) fn disk_append(&mut self, edge: u32, vert: u32) {
        let first = self.verts.records[vert as usize].edge;
        if first == NONE {
            self.verts.records[vert as usize].edge = edge;
            *self.disk_link_mut(edge, vert) = DiskLink {
                prev: edge,
                next: edge,
            };
            return;
        }
        let last = self.disk_prev_raw(first, vert);
        *self.disk_link_mut(edge, vert) = DiskLink {
            prev: last,
            next: first,
        };
        self.disk_link_mut(first, vert).prev = edge;
        self.disk_link_mut(last, vert).next = edge;
    }

    pub(crate) fn disk_remove(&mut self, edge: u32, vert: u32) {
        let link = *self.disk_link_mut(edge, vert);
        self.disk_link_mut(link.prev, vert).next = link.next;
        self.disk_link_mut(link.next, vert).prev = link.prev;
        let record = &mut self.verts.records[vert as usize];
        if record.edge == edge {
            record.edge = if link.next != edge { link.next } else { NONE };
        }
        *self.disk_link_mut(edge, vert) = DiskLink {
            prev: NONE,
            next: NONE,
        };
    }

    pub(crate) fn radial_append(&mut self, edge: u32, lp: u32) {
        let first = self.edges.records[edge as usize].first_loop;
        if first == NONE {
            let record = &mut self.loops.records[lp as usize];
            record.radial_next = lp;
            record.radial_prev = lp;
        } else {
            let after = self.loops.records[first as usize].radial_next;
            {
                let record = &mut self.loops.records[lp as usize];
                record.radial_prev = first;
                record.radial_next = after;
            }
            self.loops.records[after as usize].radial_prev = lp;
            self.loops.records[first as usize].radial_next = lp;
        }
        self.edges.records[edge as usize].first_loop = lp;
        self.loops.records[lp as usize].edge = edge;
    }

    pub(crate) fn radial_remove(&mut self, edge: u32, lp: u32) {
        let record = self.loops.records[lp as usize];
        debug_assert_eq!(record.edge, edge, "loop is not in the radial cycle");
        let edge_record = &mut self.edges.records[edge as usize];
        if record.radial_next != lp {
            if edge_record.first_loop == lp {
                edge_record.first_loop = record.radial_next;
            }
            self.loops.records[record.radial_next as usize].radial_prev = record.radial_prev;
            self.loops.records[record.radial_prev as usize].radial_next = record.radial_next;
        } else if edge_record.first_loop == lp {
            edge_record.first_loop = NONE;
        }
        let record = &mut self.loops.records[lp as usize];
        record.radial_next = NONE;
        record.radial_prev = NONE;
        record.edge = NONE;
    }

    pub(crate) fn alloc_vert(&mut self, position: Vec3) -> u32 {
        let slot = self.verts.alloc(VertRecord {
            edge: NONE,
            flags: Flags::empty(),
        });
        let index = slot.index();
        match slot {
            Slot::Fresh(_) => {
                self.positions.push(position);
                self.vert_normals.push(Vec3::ZERO);
                self.vert_data.push_default(index as usize);
            }
            Slot::Reused(_) => {
                self.positions[index as usize] = position;
                self.vert_normals[index as usize] = Vec3::ZERO;
                self.vert_data.reset(index as usize);
            }
        }
        index
    }

    pub(crate) fn alloc_edge(&mut self, a: u32, b: u32) -> u32 {
        let slot = self.edges.alloc(EdgeRecord {
            verts: [a, b],
            disk: [DiskLink::default(); 2],
            first_loop: NONE,
            flags: Flags::empty(),
        });
        let index = slot.index();
        match slot {
            Slot::Fresh(_) => self.edge_data.push_default(index as usize),
            Slot::Reused(_) => self.edge_data.reset(index as usize),
        }
        self.disk_append(index, a);
        self.disk_append(index, b);
        index
    }

    pub(crate) fn alloc_loop(&mut self, vert: u32, edge: u32, face: u32) -> u32 {
        let slot = self.loops.alloc(LoopRecord::new(vert, edge, face));
        let index = slot.index();
        match slot {
            Slot::Fresh(_) => self.loop_data.push_default(index as usize),
            Slot::Reused(_) => self.loop_data.reset(index as usize),
        }
        index
    }

    pub(crate) fn alloc_face(&mut self) -> u32 {
        let slot = self.faces.alloc(FaceRecord {
            first: NONE,
            len: 0,
            material: 0,
            flags: Flags::empty(),
        });
        let index = slot.index();
        match slot {
            Slot::Fresh(_) => {
                self.face_normals.push(Vec3::ZERO);
                self.face_data.push_default(index as usize);
            }
            Slot::Reused(_) => {
                self.face_normals[index as usize] = Vec3::ZERO;
                self.face_data.reset(index as usize);
            }
        }
        index
    }

    pub(crate) fn release_vert(&mut self, vert: u32) {
        self.forget(Element::Vert(crate::VertId(vert)));
        self.verts.release(vert);
    }

    pub(crate) fn release_edge(&mut self, edge: u32) {
        self.forget(Element::Edge(crate::EdgeId(edge)));
        self.edges.release(edge);
    }

    pub(crate) fn release_loop(&mut self, lp: u32) {
        self.loops.release(lp);
    }

    pub(crate) fn release_face(&mut self, face: u32) {
        if self.active_face == Some(face) {
            self.active_face = None;
        }
        self.forget(Element::Face(crate::FaceId(face)));
        self.faces.release(face);
    }

    fn forget(&mut self, element: Element) {
        if !self.history.is_empty() {
            self.history.retain(|stored| *stored != element);
        }
    }

    pub(crate) fn link_face_loops(&mut self, face: u32, verts: &[u32], edges: &[u32]) {
        let mut first = NONE;
        let mut last = NONE;
        for (&vert, &edge) in verts.iter().zip(edges) {
            let lp = self.alloc_loop(vert, edge, face);
            self.radial_append(edge, lp);
            if first == NONE {
                first = lp;
            } else {
                self.loops.records[last as usize].next = lp;
                self.loops.records[lp as usize].prev = last;
            }
            last = lp;
        }
        self.loops.records[last as usize].next = first;
        self.loops.records[first as usize].prev = last;
        let record = &mut self.faces.records[face as usize];
        record.first = first;
        record.len = verts.len() as u32;
    }
}
