use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use rikka_wayland_client::Route;
use rikka_wayland_client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use rikka_wayland_client::ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1;
use rikka_wayland_client::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1;
use rikka_wayland_client::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;

use crate::buffer::CaptureBuffer;
use crate::constraints::{Constraints, DmabufFormat, DmabufFormats};
use crate::error::{Error, Result};
use crate::format::PixelFormat;
use crate::pool::BufferPool;
use crate::source::Region;
use crate::state::{cursor_event, frame_event, session_event};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureEvent {
    ConstraintsChanged,
    Ready(FrameReady),
    Failed(FrameFailure),
    Stopped,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameReady {
    pub presented: Option<Duration>,
    pub damage: Vec<Region>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Captured {
    Ready {
        index: usize,
        presented: Option<Duration>,
    },
    Lost,
    Ended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameFailure {
    BufferConstraints,
    Stopped,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
    pub hotspot_x: i32,
    pub hotspot_y: i32,
}

pub struct Frames {
    session: ExtImageCopyCaptureSessionV1,
    source: Option<ExtImageCaptureSourceV1>,
    slot: Arc<Slot>,
    frame: Option<ExtImageCopyCaptureFrameV1>,
}

impl Frames {
    pub(crate) fn new(
        session: ExtImageCopyCaptureSessionV1,
        source: Option<ExtImageCaptureSourceV1>,
        slot: Arc<Slot>,
    ) -> Frames {
        let frames = Arc::clone(&slot);
        let frames = Route::tap::<ExtImageCopyCaptureFrameV1>(move |_, event| {
            frame_event(&frames, event);
        });
        Frames {
            session: session.via(&frames),
            source,
            slot,
            frame: None,
        }
    }

    #[must_use]
    pub fn constraints(&self) -> Constraints {
        self.slot.constraints()
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.slot.generation()
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.slot.is_stopped()
    }

    #[must_use]
    pub fn is_capturing(&self) -> bool {
        self.frame.is_some()
    }

    pub fn capture(&mut self, buffer: &CaptureBuffer) -> Result<()> {
        if self.frame.is_some() {
            return Err(Error::Failed("frame already requested"));
        }
        let frame = self.session.create_frame();
        frame.attach_buffer(buffer.wl());
        frame.damage_buffer(0, 0, buffer.width() as i32, buffer.height() as i32);
        frame.capture();
        self.frame = Some(frame);
        Ok(())
    }

    pub fn cancel(&mut self) {
        if let Some(frame) = self.frame.take() {
            frame.destroy();
        }
        self.slot.forget();
    }

    pub fn events(&mut self) -> Vec<CaptureEvent> {
        let events = self.slot.drain();
        if events.iter().any(is_final) {
            self.cancel();
        }
        events
    }

    pub fn collect(&mut self, mut pool: Option<&mut BufferPool>) -> Vec<Captured> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                CaptureEvent::Ready(ready) => {
                    let index = pool.as_deref_mut()?.landed()?;
                    Some(Captured::Ready {
                        index,
                        presented: ready.presented,
                    })
                }
                CaptureEvent::ConstraintsChanged
                | CaptureEvent::Failed(FrameFailure::BufferConstraints | FrameFailure::Unknown) => {
                    if let Some(pool) = pool.as_deref_mut() {
                        pool.lost();
                    }
                    Some(Captured::Lost)
                }
                CaptureEvent::Failed(FrameFailure::Stopped) | CaptureEvent::Stopped => {
                    if let Some(pool) = pool.as_deref_mut() {
                        pool.lost();
                    }
                    Some(Captured::Ended)
                }
            })
            .collect()
    }
}

fn is_final(event: &CaptureEvent) -> bool {
    matches!(
        event,
        CaptureEvent::Ready(_) | CaptureEvent::Failed(_) | CaptureEvent::Stopped
    )
}

impl Drop for Frames {
    fn drop(&mut self) {
        self.cancel();
        self.session.destroy();
        if let Some(source) = &self.source {
            source.destroy();
        }
    }
}

impl std::fmt::Debug for Frames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frames")
            .field("constraints", &self.constraints())
            .field("capturing", &self.is_capturing())
            .finish_non_exhaustive()
    }
}

pub struct CursorFrames {
    session: ExtImageCopyCaptureCursorSessionV1,
    source: ExtImageCaptureSourceV1,
    slot: Arc<CursorSlot>,
    frames: Frames,
}

impl CursorFrames {
    pub(crate) fn new(
        session: ExtImageCopyCaptureCursorSessionV1,
        source: ExtImageCaptureSourceV1,
        slot: Arc<CursorSlot>,
        frames: Frames,
    ) -> CursorFrames {
        CursorFrames {
            session,
            source,
            slot,
            frames,
        }
    }

    pub fn frames(&mut self) -> &mut Frames {
        &mut self.frames
    }

    #[must_use]
    pub fn cursor_position(&self) -> Option<CursorPosition> {
        self.slot.current()
    }
}

impl Drop for CursorFrames {
    fn drop(&mut self) {
        self.session.destroy();
        self.source.destroy();
    }
}

impl std::fmt::Debug for CursorFrames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CursorFrames")
            .field("cursor_position", &self.cursor_position())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Slot(Mutex<FrameState>);

pub(crate) fn sessions(slot: &Arc<Slot>) -> Route {
    let slot = Arc::clone(slot);
    Route::tap::<ExtImageCopyCaptureSessionV1>(move |_, event| session_event(&slot, event))
}

pub(crate) fn cursor_sessions(slot: &Arc<CursorSlot>) -> Route {
    let slot = Arc::clone(slot);
    Route::tap::<ExtImageCopyCaptureCursorSessionV1>(move |_, event| cursor_event(&slot, event))
}

#[derive(Debug, Default)]
struct FrameState {
    stating: Statement,
    constraints: Constraints,
    frame: FrameReady,
    events: VecDeque<CaptureEvent>,
    stopped: bool,
}

#[derive(Debug, Default)]
struct Statement {
    width: u32,
    height: u32,
    shm: Vec<PixelFormat>,
    device: Option<u64>,
    formats: Vec<DmabufFormat>,
}

impl Slot {
    fn state(&self) -> rikka_sync::MutexGuard<'_, FrameState> {
        self.0.lock()
    }

    pub(crate) fn constraints(&self) -> Constraints {
        self.state().constraints.clone()
    }

    pub(crate) fn generation(&self) -> u64 {
        self.state().constraints.generation
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.state().stopped
    }

    pub(crate) fn drain(&self) -> Vec<CaptureEvent> {
        self.state().events.drain(..).collect()
    }

    pub(crate) fn size(&self, width: u32, height: u32) {
        let mut state = self.state();
        state.stating.width = width;
        state.stating.height = height;
    }

    pub(crate) fn shm_format(&self, format: PixelFormat) {
        self.state().stating.shm.push(format);
    }

    pub(crate) fn dmabuf_device(&self, device: u64) {
        self.state().stating.device = Some(device);
    }

    pub(crate) fn dmabuf_format(&self, format: PixelFormat, modifiers: Vec<u64>) {
        self.state()
            .stating
            .formats
            .push(DmabufFormat { format, modifiers });
    }

    pub(crate) fn stated(&self) {
        let mut state = self.state();
        let stating = std::mem::take(&mut state.stating);
        let generation = state.constraints.generation + 1;
        state.constraints = Constraints {
            width: stating.width,
            height: stating.height,
            shm: stating.shm,
            dmabuf: stating
                .device
                .filter(|_| !stating.formats.is_empty())
                .map(|device| DmabufFormats {
                    device,
                    formats: stating.formats,
                }),
            generation,
        };
        state.events.push_back(CaptureEvent::ConstraintsChanged);
    }

    pub(crate) fn damaged(&self, region: Region) {
        self.state().frame.damage.push(region);
    }

    pub(crate) fn presented(&self, at: Duration) {
        self.state().frame.presented = Some(at);
    }

    pub(crate) fn ready(&self) {
        let mut state = self.state();
        let frame = std::mem::take(&mut state.frame);
        state.events.push_back(CaptureEvent::Ready(frame));
    }

    pub(crate) fn forget(&self) {
        self.state().frame = FrameReady::default();
    }

    pub(crate) fn failed(&self, failure: FrameFailure) {
        let mut state = self.state();
        state.frame = FrameReady::default();
        state.events.push_back(CaptureEvent::Failed(failure));
    }

    pub(crate) fn stopped(&self) {
        let mut state = self.state();
        state.stopped = true;
        state.events.push_back(CaptureEvent::Stopped);
    }
}

#[derive(Debug, Default)]
pub(crate) struct CursorSlot(Mutex<CursorState>);

#[derive(Debug, Default)]
struct CursorState {
    entered: bool,
    at: CursorPosition,
}

impl CursorSlot {
    fn position_mut(&self) -> rikka_sync::MutexGuard<'_, CursorState> {
        self.0.lock()
    }

    pub(crate) fn current(&self) -> Option<CursorPosition> {
        let cursor = self.position_mut();
        cursor.entered.then_some(cursor.at)
    }

    pub(crate) fn entered(&self, entered: bool) {
        self.position_mut().entered = entered;
    }

    pub(crate) fn position(&self, x: i32, y: i32) {
        let mut position = self.position_mut();
        position.at.x = x;
        position.at.y = y;
    }

    pub(crate) fn hotspot(&self, x: i32, y: i32) {
        let mut position = self.position_mut();
        position.at.hotspot_x = x;
        position.at.hotspot_y = y;
    }
}
