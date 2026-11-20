use std::fmt;

use rikka_compositor_geometry::{Buffer, Region, Size};
use rikka_compositor_protocols::ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1;
use rikka_compositor_protocols::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1;
use rikka_compositor_protocols::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
use rikka_compositor_protocols::wl_buffer::WlBuffer;
use rikka_compositor_protocols::wl_pointer::WlPointer;
use slotmap::SlotMap;

use super::types::{CaptureSource, Constraints, CursorPlacement, SessionInfo};
use crate::ids::{CaptureFrameId, CaptureSessionId, CursorSessionId};

pub struct Captures<T> {
    pub(super) sessions: SlotMap<CaptureSessionId, Session<T>>,
    pub(super) frames: SlotMap<CaptureFrameId, Frame>,
    pub(super) cursors: SlotMap<CursorSessionId, Cursor<T>>,
}

pub(super) struct Session<T> {
    pub(super) resource: ExtImageCopyCaptureSessionV1,
    pub(super) info: SessionInfo<T>,
    pub(super) constraints: Option<Constraints>,
    pub(super) frame: Option<CaptureFrameId>,
    pub(super) delivered: bool,
    pub(super) stopped: bool,
}

pub(super) struct Frame {
    pub(super) resource: ExtImageCopyCaptureFrameV1,
    pub(super) session: Option<CaptureSessionId>,
    pub(super) buffer: Option<WlBuffer>,
    pub(super) damage: Region<Buffer>,
    pub(super) phase: Phase,
}

pub(super) enum Phase {
    Preparing,
    Capturing {
        session: CaptureSessionId,
        size: Size<i32, Buffer>,
    },
    Finished,
}

pub(super) struct Cursor<T> {
    pub(super) resource: ExtImageCopyCaptureCursorSessionV1,
    pub(super) source: Option<CaptureSource<T>>,
    pub(super) pointer: WlPointer,
    pub(super) session_taken: bool,
    pub(super) placement: Option<CursorPlacement>,
}

impl<T: Copy + PartialEq> Captures<T> {
    pub fn new() -> Self {
        Self {
            sessions: SlotMap::with_key(),
            frames: SlotMap::with_key(),
            cursors: SlotMap::with_key(),
        }
    }

    pub fn session(&self, session: CaptureSessionId) -> Option<SessionInfo<T>> {
        self.sessions.get(session).map(|entry| entry.info)
    }

    pub fn sessions(&self) -> impl Iterator<Item = (CaptureSessionId, SessionInfo<T>)> + '_ {
        self.sessions
            .iter()
            .filter(|(_, entry)| !entry.stopped)
            .map(|(id, entry)| (id, entry.info))
    }

    pub fn constraints(&self, session: CaptureSessionId) -> Option<&Constraints> {
        self.sessions.get(session)?.constraints.as_ref()
    }

    pub fn is_pending(&self, frame: CaptureFrameId) -> bool {
        self.frames
            .get(frame)
            .is_some_and(|frame| matches!(frame.phase, Phase::Capturing { .. }))
    }

    pub fn cursor_sessions(
        &self,
    ) -> impl Iterator<Item = (CursorSessionId, CaptureSource<T>, &WlPointer)> {
        self.cursors
            .iter()
            .filter_map(|(id, cursor)| cursor.source.map(|source| (id, source, &cursor.pointer)))
    }
}

impl<T: Copy + PartialEq> Default for Captures<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Captures<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Captures")
            .field("sessions", &self.sessions.len())
            .field("frames", &self.frames.len())
            .field("cursors", &self.cursors.len())
            .finish()
    }
}
