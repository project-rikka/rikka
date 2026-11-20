use std::sync::atomic::{AtomicU64, Ordering};

use crate::ids::{EdgeId, FaceId, LoopId, VertId};
use crate::mesh::{EditMesh, Flags};

const MIN_LIMIT: usize = 1 << 16;
const LIMIT_SHARE: usize = 8;
const WATCHED: Flags = Flags::SELECT.union(Flags::HIDDEN);

static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

fn next_epoch() -> u64 {
    NEXT_EPOCH.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug)]
pub(crate) struct Journal {
    epoch: u64,
    corner_epoch: u64,
    moved: Vec<VertId>,
    verts: Vec<VertId>,
    edges: Vec<EdgeId>,
    faces: Vec<FaceId>,
    corners: Vec<LoopId>,
}

impl Default for Journal {
    fn default() -> Journal {
        Journal {
            epoch: next_epoch(),
            corner_epoch: next_epoch(),
            moved: Vec::new(),
            verts: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            corners: Vec::new(),
        }
    }
}

impl Clone for Journal {
    fn clone(&self) -> Journal {
        Journal::default()
    }
}

impl Journal {
    pub(crate) fn reset(&mut self) {
        self.epoch = next_epoch();
        self.moved.clear();
        self.verts.clear();
        self.edges.clear();
        self.faces.clear();
        self.forget_corners();
    }

    pub(crate) fn forget_corners(&mut self) {
        self.corner_epoch = next_epoch();
        self.corners.clear();
    }

    fn element_len(&self) -> usize {
        self.moved.len() + self.verts.len() + self.edges.len() + self.faces.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JournalMark {
    epoch: u64,
    corner_epoch: u64,
    moved: usize,
    verts: usize,
    edges: usize,
    faces: usize,
    corners: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Changes<'a> {
    pub moved: &'a [VertId],
    pub verts: &'a [VertId],
    pub edges: &'a [EdgeId],
    pub faces: &'a [FaceId],
    pub corners: Option<&'a [LoopId]>,
}

impl Changes<'_> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.moved.len()
            + self.verts.len()
            + self.edges.len()
            + self.faces.len()
            + self.corners.map_or(0, <[LoopId]>::len)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn flag_len(&self) -> usize {
        self.verts.len() + self.edges.len() + self.faces.len()
    }
}

impl EditMesh {
    #[must_use]
    pub fn journal_mark(&self) -> JournalMark {
        JournalMark {
            epoch: self.journal.epoch,
            corner_epoch: self.journal.corner_epoch,
            moved: self.journal.moved.len(),
            verts: self.journal.verts.len(),
            edges: self.journal.edges.len(),
            faces: self.journal.faces.len(),
            corners: self.journal.corners.len(),
        }
    }

    #[must_use]
    pub fn changes_since(&self, mark: JournalMark) -> Option<Changes<'_>> {
        let journal = &self.journal;
        let valid = mark.epoch == journal.epoch
            && mark.moved <= journal.moved.len()
            && mark.verts <= journal.verts.len()
            && mark.edges <= journal.edges.len()
            && mark.faces <= journal.faces.len();
        let corners = (mark.corner_epoch == journal.corner_epoch
            && mark.corners <= journal.corners.len())
        .then(|| &journal.corners[mark.corners..]);
        valid.then(|| Changes {
            moved: &journal.moved[mark.moved..],
            verts: &journal.verts[mark.verts..],
            edges: &journal.edges[mark.edges..],
            faces: &journal.faces[mark.faces..],
            corners,
        })
    }

    fn journal_limit(&self) -> usize {
        let capacity = self.verts.capacity() + self.edges.capacity() + self.faces.capacity();
        MIN_LIMIT.max(capacity / LIMIT_SHARE)
    }

    fn make_room(&mut self) {
        if self.journal.element_len() >= self.journal_limit() {
            self.journal.reset();
        }
    }

    pub(crate) fn log_moved(&mut self, vert: VertId) {
        self.make_room();
        self.journal.moved.push(vert);
    }

    pub(crate) fn log_corner(&mut self, lp: LoopId) {
        if self.journal.corners.len() >= MIN_LIMIT.max(self.loops.capacity() / LIMIT_SHARE) {
            self.journal.forget_corners();
        }
        self.journal.corners.push(lp);
    }

    pub(crate) fn forget_corners(&mut self) {
        self.journal.forget_corners();
    }

    pub(crate) fn put_vert_flags(&mut self, vert: VertId, flags: Flags) {
        let record = &mut self.verts.records[vert.index()];
        let changed = (record.flags ^ flags).intersects(WATCHED);
        record.flags = flags;
        if changed {
            self.make_room();
            self.journal.verts.push(vert);
        }
    }

    pub(crate) fn put_edge_flags(&mut self, edge: EdgeId, flags: Flags) {
        let record = &mut self.edges.records[edge.index()];
        let changed = (record.flags ^ flags).intersects(WATCHED);
        record.flags = flags;
        if changed {
            self.make_room();
            self.journal.edges.push(edge);
        }
    }

    pub(crate) fn put_face_flags(&mut self, face: FaceId, flags: Flags) {
        let record = &mut self.faces.records[face.index()];
        let changed = (record.flags ^ flags).intersects(WATCHED);
        record.flags = flags;
        if changed {
            self.make_room();
            self.journal.faces.push(face);
        }
    }

    pub(crate) fn mark_vert(&mut self, vert: VertId, flag: Flags, enabled: bool) {
        let mut flags = self.verts.records[vert.index()].flags;
        flags.set(flag, enabled);
        self.put_vert_flags(vert, flags);
    }

    pub(crate) fn mark_edge(&mut self, edge: EdgeId, flag: Flags, enabled: bool) {
        let mut flags = self.edges.records[edge.index()].flags;
        flags.set(flag, enabled);
        self.put_edge_flags(edge, flags);
    }

    pub(crate) fn mark_face(&mut self, face: FaceId, flag: Flags, enabled: bool) {
        let mut flags = self.faces.records[face.index()].flags;
        flags.set(flag, enabled);
        self.put_face_flags(face, flags);
    }
}
