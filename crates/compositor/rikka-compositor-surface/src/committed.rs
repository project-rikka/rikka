use bitflags::bitflags;
use rikka_compositor_geometry::{Logical, Point, Size};

use crate::ids::SurfaceId;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Changes: u32 {
        const SIZE = 1;
        const DAMAGE = 1 << 1;
        const OPAQUE = 1 << 2;
        const INPUT = 1 << 3;
        const SCALE = 1 << 4;
        const TRANSFORM = 1 << 5;
        const VIEWPORT = 1 << 6;
        const OFFSET = 1 << 7;
        const CHILDREN = 1 << 8;
        const ALPHA = 1 << 9;
        const CONTENT_TYPE = 1 << 10;
        const TEARING = 1 << 11;
        const FRAME_CALLBACKS = 1 << 12;
        const FEEDBACK = 1 << 13;
        const COLOR = 1 << 14;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BufferChange {
    #[default]
    Unchanged,
    Attached,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Committed {
    pub surface: SurfaceId,
    pub sequence: u64,
    pub buffer: BufferChange,
    pub offset: Point<i32, Logical>,
    pub changes: Changes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreCommit {
    pub surface: SurfaceId,
    pub buffer: BufferChange,
    pub size: Option<Size<i32, Logical>>,
    pub synced: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rejected;
