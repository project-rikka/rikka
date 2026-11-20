use rikka_types::Rect;

use crate::draw::arena::Span;
use crate::scene::{LayerId, NodeId};

pub(crate) const CHUNK_INSTANCES: u32 = 256;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Segment {
    Draw {
        first: u32,
        count: u32,
    },
    Backdrop {
        node: NodeId,
        instance: u32,
        region: Rect,
    },
    Capture {
        region: Rect,
    },
    Fade {
        layer: LayerId,
        opacity: f32,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Chunk {
    pub first: u32,
    pub count: u32,
    pub bounds: Rect,
    pub members: (u32, u32),
    pub id: u32,
    pub has_textured: bool,
}

#[derive(Default)]
pub(crate) struct Run {
    open: bool,
    first_chunk: u32,
    chunks: u32,
    first: u32,
    count: u32,
    bounds: Rect,
    members: (u32, u32),
    has_textured: bool,
}

impl Run {
    pub(crate) fn take(
        &mut self,
        chunks: &mut Vec<Chunk>,
        span: Span,
        bounds: Rect,
        textured: bool,
        member: u32,
    ) {
        if span.room == 0 {
            return;
        }
        let follows = self.open
            && self.first + self.count == span.first
            && self.count + span.room <= CHUNK_INSTANCES;
        if !follows {
            self.close(chunks);
            self.open = true;
            self.first = span.first;
            self.count = 0;
            self.bounds = Rect::ZERO;
            self.members = (member, member);
            self.has_textured = false;
        }
        if !bounds.is_empty() {
            self.bounds = match self.bounds.is_empty() {
                true => bounds,
                false => self.bounds.union(&bounds),
            };
        }
        self.has_textured |= textured;
        self.count += span.room;
        self.members.1 = member + 1;
    }

    pub(crate) fn close(&mut self, chunks: &mut Vec<Chunk>) {
        if self.open && self.count > 0 {
            chunks.push(Chunk {
                id: chunks.len() as u32,
                first: self.first,
                count: self.count,
                bounds: self.bounds,
                members: self.members,
                has_textured: self.has_textured,
            });
            self.chunks += 1;
        }
        self.open = false;
        self.count = 0;
        self.has_textured = false;
    }

    pub(crate) fn flush(&mut self, chunks: &mut Vec<Chunk>, segments: &mut Vec<Segment>) {
        self.close(chunks);
        if self.chunks == 0 {
            return;
        }
        segments.push(Segment::Draw {
            first: self.first_chunk,
            count: self.chunks,
        });
        self.first_chunk = chunks.len() as u32;
        self.chunks = 0;
    }
}

pub(crate) fn too_scattered(chunks: usize, segments: usize, instances: u32) -> bool {
    let least = segments + instances as usize / CHUNK_INSTANCES as usize;
    chunks > least + 64
}
