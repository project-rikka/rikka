use std::sync::atomic::{AtomicU64, Ordering};

use bitflags::bitflags;
use glam::Vec3;
use rikka_polymesh::{Domain, Layers, VertexGroup};

use crate::arena::Arena;
use crate::data::DataTable;
use crate::ids::{Element, NONE};
use crate::journal::Journal;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Flags: u8 {
        const SELECT = 1;
        const HIDDEN = 1 << 1;
        const SEAM = 1 << 2;
        const SHARP = 1 << 3;
        const SMOOTH = 1 << 4;
        const TAG = 1 << 5;
    }
}

impl Flags {
    const MERGED: Flags = Flags::SELECT
        .union(Flags::HIDDEN)
        .union(Flags::SEAM)
        .union(Flags::SMOOTH)
        .union(Flags::TAG);

    #[must_use]
    pub fn merge(self, other: Flags) -> Flags {
        ((self | other) & Flags::MERGED) | (self & other & Flags::SHARP)
    }

    #[must_use]
    pub fn merge_hidden(self, other: Flags) -> Flags {
        let mut merged = self.merge(other);
        if !(self & other).contains(Flags::HIDDEN) {
            merged.remove(Flags::HIDDEN);
        }
        merged
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SelectMode: u8 {
        const VERT = 1;
        const EDGE = 1 << 1;
        const FACE = 1 << 2;
    }
}

impl Default for SelectMode {
    fn default() -> SelectMode {
        SelectMode::VERT
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Revisions {
    pub mesh: u64,
    pub topology: u64,
    pub shape: u64,
    pub selection: u64,
    pub data: u64,
    pub keys: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyBlock {
    pub name: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub relative_to: u32,
    pub vertex_group: Option<String>,
    pub muted: bool,
}

impl KeyBlock {
    pub fn new(name: impl Into<String>) -> KeyBlock {
        KeyBlock {
            name: name.into(),
            value: 0.0,
            min: 0.0,
            max: 1.0,
            relative_to: 0,
            vertex_group: None,
            muted: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VertRecord {
    pub(crate) edge: u32,
    pub(crate) flags: Flags,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DiskLink {
    pub(crate) prev: u32,
    pub(crate) next: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeRecord {
    pub(crate) verts: [u32; 2],
    pub(crate) disk: [DiskLink; 2],
    pub(crate) first_loop: u32,
    pub(crate) flags: Flags,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LoopRecord {
    pub(crate) vert: u32,
    pub(crate) edge: u32,
    pub(crate) face: u32,
    pub(crate) next: u32,
    pub(crate) prev: u32,
    pub(crate) radial_next: u32,
    pub(crate) radial_prev: u32,
    pub(crate) flags: Flags,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceRecord {
    pub(crate) first: u32,
    pub(crate) len: u32,
    pub(crate) material: i32,
    pub(crate) flags: Flags,
}

impl LoopRecord {
    pub(crate) fn new(vert: u32, edge: u32, face: u32) -> LoopRecord {
        LoopRecord {
            vert,
            edge,
            face,
            next: NONE,
            prev: NONE,
            radial_next: NONE,
            radial_prev: NONE,
            flags: Flags::empty(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Layout {
    pub(crate) uv_maps: Layers,
    pub(crate) color_attributes: Layers,
    pub(crate) vertex_groups: Vec<VertexGroup>,
    pub(crate) active_vertex_group: Option<usize>,
    pub(crate) key_blocks: Vec<KeyBlock>,
    pub(crate) active_key_block: Option<usize>,
    pub(crate) attribute_order: Vec<String>,
}

static NEXT_STAMP: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(crate) struct Stamp(pub(crate) u64);

impl Stamp {
    pub(crate) fn new() -> Stamp {
        Stamp(NEXT_STAMP.fetch_add(1, Ordering::Relaxed))
    }
}

impl Clone for Stamp {
    fn clone(&self) -> Stamp {
        Stamp::new()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Counters {
    pub(crate) topology: u64,
    pub(crate) shape: u64,
    pub(crate) selection: u64,
    pub(crate) data: u64,
    pub(crate) keys: u64,
}

#[derive(Clone, Debug)]
pub struct EditMesh {
    pub(crate) verts: Arena<VertRecord>,
    pub(crate) edges: Arena<EdgeRecord>,
    pub(crate) loops: Arena<LoopRecord>,
    pub(crate) faces: Arena<FaceRecord>,
    pub(crate) positions: Vec<Vec3>,
    pub(crate) vert_normals: Vec<Vec3>,
    pub(crate) face_normals: Vec<Vec3>,
    pub(crate) vert_data: DataTable,
    pub(crate) edge_data: DataTable,
    pub(crate) loop_data: DataTable,
    pub(crate) face_data: DataTable,
    pub(crate) layout: Layout,
    pub(crate) select_mode: SelectMode,
    pub(crate) history: Vec<Element>,
    pub(crate) active_face: Option<u32>,
    pub(crate) stamp: Stamp,
    pub(crate) counters: Counters,
    pub(crate) journal: Journal,
}

impl Default for EditMesh {
    fn default() -> EditMesh {
        EditMesh {
            verts: Arena::default(),
            edges: Arena::default(),
            loops: Arena::default(),
            faces: Arena::default(),
            positions: Vec::new(),
            vert_normals: Vec::new(),
            face_normals: Vec::new(),
            vert_data: DataTable::new(Domain::Point),
            edge_data: DataTable::new(Domain::Edge),
            loop_data: DataTable::new(Domain::Corner),
            face_data: DataTable::new(Domain::Face),
            layout: Layout::default(),
            select_mode: SelectMode::default(),
            history: Vec::new(),
            active_face: None,
            stamp: Stamp::new(),
            counters: Counters::default(),
            journal: Journal::default(),
        }
    }
}

impl EditMesh {
    #[must_use]
    pub fn new() -> EditMesh {
        EditMesh::default()
    }

    #[must_use]
    pub fn revisions(&self) -> Revisions {
        Revisions {
            mesh: self.stamp.0,
            topology: self.counters.topology,
            shape: self.counters.shape,
            selection: self.counters.selection,
            data: self.counters.data,
            keys: self.counters.keys,
        }
    }

    pub(crate) fn touch_topology(&mut self) {
        self.counters.topology += 1;
        self.journal.reset();
    }

    pub(crate) fn touch_shape(&mut self) {
        self.counters.shape += 1;
        self.journal.reset();
    }

    pub(crate) fn touch_moved(&mut self) {
        self.counters.shape += 1;
    }

    pub(crate) fn touch_selection(&mut self) {
        self.counters.selection += 1;
    }

    pub(crate) fn touch_data(&mut self) {
        self.counters.data += 1;
    }

    pub(crate) fn touch_keys(&mut self) {
        self.counters.keys += 1;
        self.touch_shape();
    }

    pub(crate) fn table(&self, domain: Domain) -> &DataTable {
        match domain {
            Domain::Point => &self.vert_data,
            Domain::Edge => &self.edge_data,
            Domain::Face => &self.face_data,
            Domain::Corner => &self.loop_data,
        }
    }

    pub(crate) fn table_mut(&mut self, domain: Domain) -> &mut DataTable {
        match domain {
            Domain::Point => &mut self.vert_data,
            Domain::Edge => &mut self.edge_data,
            Domain::Face => &mut self.face_data,
            Domain::Corner => &mut self.loop_data,
        }
    }

    pub(crate) fn capacity(&self, domain: Domain) -> usize {
        match domain {
            Domain::Point => self.verts.capacity(),
            Domain::Edge => self.edges.capacity(),
            Domain::Face => self.faces.capacity(),
            Domain::Corner => self.loops.capacity(),
        }
    }
}
