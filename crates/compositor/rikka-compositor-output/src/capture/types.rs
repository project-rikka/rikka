use std::rc::Rc;
use std::time::Duration;

use rikka_compositor_geometry::{Buffer, Point, Region, Size, Transform};
use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_types::{Fourcc, Modifier, OutputId};

use crate::ids::{CaptureFrameId, CaptureSessionId, CursorSessionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureSource<T> {
    Output(OutputId),
    Toplevel(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionInfo<T> {
    pub source: CaptureSource<T>,
    pub paint_cursors: bool,
    pub cursor: Option<CursorSessionId>,
}

#[derive(Debug, Clone, Eq)]
pub struct Constraints {
    pub size: Size<i32, Buffer>,
    pub formats: Rc<CaptureFormats>,
}

impl PartialEq for Constraints {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && (Rc::ptr_eq(&self.formats, &other.formats) || self.formats == other.formats)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureFormats {
    pub shm: Box<[Fourcc]>,
    pub dmabuf: Option<DmabufConstraints>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmabufConstraints {
    pub device: u64,
    pub formats: Box<[DmabufFormat]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmabufFormat {
    pub fourcc: Fourcc,
    pub modifiers: Box<[Modifier]>,
}

#[derive(Debug, Clone)]
pub struct CaptureRequest<T> {
    pub frame: CaptureFrameId,
    pub session: CaptureSessionId,
    pub info: SessionInfo<T>,
    pub size: Size<i32, Buffer>,
    pub buffer: WlBuffer,
    pub damage: Region<Buffer>,
}

#[derive(Debug, Clone)]
pub struct FrameReady {
    pub presented: Duration,
    pub transform: Transform,
    pub damage: Region<Buffer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPlacement {
    pub position: Point<i32, Buffer>,
    pub hotspot: Point<i32, Buffer>,
}
