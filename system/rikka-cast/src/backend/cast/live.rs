use std::collections::HashMap;

use libspa::param::ParamType;
use libspa::pod::Pod;
use pipewire::stream::StreamRc;
use pipewire::sys as pw_sys;
use rikka_capture::{
    CaptureBuffer, CaptureEvent, Constraints, CursorFrames, FrameFailure, FrameReady, Frames,
    Screen,
};

use crate::backend::cast::CursorMode;
use crate::backend::cast::format::{self, Negotiated};
use crate::backend::cast::meta::{self, Cursor, CursorBitmap, CursorPosition, Frame};
use crate::error::Result;
use crate::protocol::CastSource;

const MAX_GPU_FAILURES: u32 = 3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    pub(crate) position: Option<(i32, i32)>,
    pub(crate) size: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeState {
    Pending,
    Created { node: u32 },
    FrameReady { node: u32, serial: u64 },
}

impl NodeState {
    fn ready(self) -> Option<(u32, u64)> {
        match self {
            Self::FrameReady { node, serial } => Some((node, serial)),
            Self::Pending | Self::Created { .. } => None,
        }
    }

    fn placed(self, node: u32) -> Self {
        match self {
            Self::FrameReady { node: current, .. } if current == node => self,
            _ => Self::Created { node },
        }
    }

    fn negotiated(self, serials: &HashMap<u32, u64>) -> Self {
        match self {
            Self::Created { node } => serials.get(&node).map_or(self, |serial| Self::FrameReady {
                node,
                serial: *serial,
            }),
            Self::Pending | Self::FrameReady { .. } => self,
        }
    }
}

pub(crate) struct CastStreamState {
    pub(crate) source: CastSource,
    pub(crate) place: Placement,
    stream: StreamRc,
    frames: Frames,
    cursor: Option<CursorStream>,
    constraints: Constraints,
    negotiated: Option<Negotiated>,
    buffers: HashMap<usize, Allocated>,
    drawing: Option<*mut pw_sys::pw_buffer>,
    sequence: u64,
    presence: NodeState,
    gpu_failures: u32,
    shm_only: bool,
}

struct Allocated {
    generation: u64,
    buffer: CaptureBuffer,
}

struct CursorStream {
    frames: CursorFrames,
    buffer: Option<CaptureBuffer>,
    generation: u64,
    is_fresh: bool,
}

impl CastStreamState {
    pub(crate) fn open(
        screen: &mut Screen,
        stream: StreamRc,
        source: CastSource,
        place: Placement,
        selection: &rikka_capture::Source,
        cursor: CursorMode,
    ) -> Result<CastStreamState> {
        let mut frames = screen.frames(selection, cursor == CursorMode::Embedded)?;
        let constraints = screen.negotiate(&mut frames)?;

        let cursor = match cursor == CursorMode::Metadata {
            false => None,
            true => match screen.cursor_frames(selection) {
                Ok(frames) => Some(CursorStream {
                    frames,
                    buffer: None,
                    generation: 0,
                    is_fresh: false,
                }),
                Err(error) => {
                    eprintln!(
                        "rikka-portal-cast: {}: no cursor capture: {error}",
                        source.id
                    );
                    None
                }
            },
        };

        Ok(CastStreamState {
            source,
            place,
            stream,
            frames,
            cursor,
            constraints,
            negotiated: None,
            buffers: HashMap::new(),
            drawing: None,
            sequence: 0,
            presence: NodeState::Pending,
            gpu_failures: 0,
            shm_only: false,
        })
    }

    pub(crate) fn stream(&self) -> &StreamRc {
        &self.stream
    }

    pub(crate) fn constraints(&self) -> &Constraints {
        &self.constraints
    }

    pub(crate) fn published(&self) -> Option<(u32, u64)> {
        self.presence.ready()
    }

    pub(crate) fn found_node(&mut self, node: u32) {
        self.presence = self.presence.placed(node);
    }

    pub(crate) fn settle_serial(&mut self, serials: &HashMap<u32, u64>) {
        self.presence = self.presence.negotiated(serials);
    }

    pub(crate) fn param_changed(&mut self, screen: &mut Screen, id: u32, param: Option<&Pod>) {
        if id != ParamType::Format.as_raw() {
            return;
        }
        let Some(param) = param else {
            self.negotiated = None;
            return;
        };
        let Some(negotiated) = format::read(param) else {
            return;
        };

        if !negotiated.modifier_choices.is_empty() {
            self.settle_modifier(screen, &negotiated);
            return;
        }

        let dmabuf = negotiated.modifier.is_some();
        let mut params = vec![format::buffers(&negotiated, dmabuf)];
        params.extend(format::metas(self.cursor.is_some()));
        self.negotiated = Some(negotiated);
        self.update(&params);
    }

    fn offered_constraints(&self) -> Constraints {
        match self.shm_only {
            true => Constraints {
                dmabuf: None,
                ..self.constraints.clone()
            },
            false => self.constraints.clone(),
        }
    }

    fn settle_modifier(&mut self, screen: &mut Screen, negotiated: &Negotiated) {
        let offered_constraints = self.offered_constraints();
        let chosen = self.choose_modifier(screen, negotiated, &offered_constraints);

        let params = match chosen {
            Some(modifier) => format::fixate_modifier(negotiated, modifier, &offered_constraints),
            None => format::offers(&Constraints {
                dmabuf: None,
                ..offered_constraints
            }),
        };
        self.update(&params);
    }

    fn choose_modifier(
        &self,
        screen: &mut Screen,
        negotiated: &Negotiated,
        offered_constraints: &Constraints,
    ) -> Option<u64> {
        let dmabuf = offered_constraints.dmabuf.as_ref()?;
        let offered: Vec<u64> = dmabuf
            .modifiers(negotiated.format)
            .iter()
            .copied()
            .filter(|modifier| negotiated.modifier_choices.contains(modifier))
            .collect();
        if offered.is_empty() {
            return None;
        }
        screen
            .probe_modifier(
                dmabuf.device,
                negotiated.format,
                &offered,
                negotiated.width,
                negotiated.height,
            )
            .ok()
    }

    pub(crate) fn add_buffer(&mut self, screen: &mut Screen, buffer: *mut pw_sys::pw_buffer) {
        let Some(negotiated) = self.negotiated.clone() else {
            return;
        };
        let allocated = match negotiated.modifier {
            Some(modifier) => self
                .constraints
                .dmabuf
                .as_ref()
                .ok_or_else(|| "no dmabuf device advertised".to_owned())
                .and_then(|dmabuf| {
                    screen
                        .dmabuf_buffer(
                            dmabuf.device,
                            negotiated.format,
                            &[modifier],
                            negotiated.width,
                            negotiated.height,
                        )
                        .map_err(|error| error.to_string())
                }),
            None => screen
                .shm_buffer(negotiated.format, negotiated.width, negotiated.height)
                .map_err(|error| error.to_string()),
        };
        let allocated = match allocated {
            Ok(allocated) => allocated,
            Err(error) => {
                eprintln!("rikka-portal-cast: {}: {error}", self.source.id);
                return;
            }
        };

        if !unsafe { meta::attach(buffer, &allocated) } {
            eprintln!(
                "rikka-portal-cast: {}: PipeWire buffer does not match capture buffer",
                self.source.id
            );
            return;
        }
        self.buffers.insert(
            buffer as usize,
            Allocated {
                generation: self.constraints.generation,
                buffer: allocated,
            },
        );
    }

    pub(crate) fn remove_buffer(&mut self, buffer: *mut pw_sys::pw_buffer) {
        if self.drawing == Some(buffer) {
            self.frames.cancel();
            self.drawing = None;
        }
        self.buffers.remove(&(buffer as usize));
    }

    pub(crate) fn pump_frames(&mut self, screen: &mut Screen) {
        self.pump_cursor(screen);
        if self.drawing.is_some()
            || self.negotiated.is_none()
            || self.buffers.is_empty()
            || self.frames.is_stopped()
        {
            return;
        }
        for _ in 0..self.buffers.len() {
            let buffer = unsafe { self.stream.dequeue_raw_buffer() };
            if buffer.is_null() {
                return;
            }
            let entry = self
                .buffers
                .get(&(buffer as usize))
                .filter(|entry| entry.generation == self.constraints.generation);
            let Some(entry) = entry else {
                unsafe {
                    meta::empty(buffer);
                    self.stream.queue_raw_buffer(buffer);
                }
                continue;
            };
            if let Err(error) = self.frames.capture(&entry.buffer) {
                eprintln!("rikka-portal-cast: {}: {error}", self.source.id);
                unsafe {
                    meta::empty(buffer);
                    self.stream.queue_raw_buffer(buffer);
                }
                return;
            }
            self.drawing = Some(buffer);
            return;
        }
    }

    pub(crate) fn handle_events(&mut self, screen: &mut Screen) -> bool {
        let mut alive = !self.frames.is_stopped();
        for event in self.frames.events() {
            match event {
                CaptureEvent::Ready(ready) => {
                    self.gpu_failures = 0;
                    self.ready(ready);
                }
                CaptureEvent::Failed(FrameFailure::Stopped) | CaptureEvent::Stopped => {
                    self.requeue_drawing();
                    alive = false;
                }
                CaptureEvent::Failed(_) => {
                    self.gpu_failures += 1;
                    self.requeue_drawing();
                }
                CaptureEvent::ConstraintsChanged => self.constraints_changed(),
            }
        }
        self.fall_back_to_shm();
        self.cursor_events();
        if alive {
            self.pump_frames(screen);
        }
        alive
    }

    fn fall_back_to_shm(&mut self) {
        if self.shm_only || self.gpu_failures < MAX_GPU_FAILURES || !self.is_dmabuf() {
            return;
        }
        eprintln!(
            "rikka-portal-cast: {}: {MAX_GPU_FAILURES} dmabuf frames failed; falling back to shm",
            self.source.id
        );
        self.shm_only = true;
        self.gpu_failures = 0;
        self.negotiated = None;
        self.frames.cancel();
        self.requeue_drawing();
        let params = format::offers(&self.offered_constraints());
        self.update(&params);
    }

    fn is_dmabuf(&self) -> bool {
        self.negotiated
            .as_ref()
            .is_some_and(|negotiated| negotiated.modifier.is_some())
    }

    fn ready(&mut self, ready: FrameReady) {
        let Some(buffer) = self.drawing.take() else {
            return;
        };
        let Some(negotiated) = self.negotiated.clone() else {
            unsafe { self.stream.queue_raw_buffer(buffer) };
            return;
        };
        let stride = self
            .buffers
            .get(&(buffer as usize))
            .map_or_else(|| negotiated.stride(), |entry| entry.buffer.stride());
        self.sequence += 1;

        let cursor = match &mut self.cursor {
            None => Cursor::NotTracked,
            Some(pointer) => match pointer.position() {
                Some(position) => Cursor::Visible(position),
                None => Cursor::Hidden,
            },
        };
        let frame = Frame {
            size: stride * negotiated.height,
            stride,
            width: negotiated.width,
            height: negotiated.height,
            presented: ready.presented,
            sequence: self.sequence,
            damage: &ready.damage,
            cursor,
        };
        unsafe { meta::resolve_source(buffer, &frame) };

        unsafe { self.stream.queue_raw_buffer(buffer) };
        if self.stream.is_driving() {
            let _ = self.stream.trigger_process();
        }
    }

    fn requeue_drawing(&mut self) {
        let Some(buffer) = self.drawing.take() else {
            return;
        };
        unsafe {
            meta::empty(buffer);
            self.stream.queue_raw_buffer(buffer);
        }
    }

    fn constraints_changed(&mut self) {
        let constraints_changed = self.frames.constraints();
        let same = constraints_changed.width == self.constraints.width
            && constraints_changed.height == self.constraints.height
            && constraints_changed.shm == self.constraints.shm
            && constraints_changed.dmabuf == self.constraints.dmabuf;
        self.constraints = constraints_changed;
        self.gpu_failures = 0;
        if same {
            return;
        }
        self.negotiated = None;
        self.frames.cancel();
        self.requeue_drawing();
        let params = format::offers(&self.constraints);
        self.update(&params);
    }

    fn update(&self, params: &[Vec<u8>]) {
        let mut pods: Vec<&Pod> = Vec::with_capacity(params.len());
        for param in params {
            match Pod::from_bytes(param) {
                Some(pod) => pods.push(pod),
                None => return,
            }
        }
        if let Err(error) = self.stream.update_params(&mut pods) {
            eprintln!(
                "rikka-portal-cast: {}: cannot update stream params: {error}",
                self.source.id
            );
        }
    }

    fn pump_cursor(&mut self, screen: &mut Screen) {
        let Some(pointer) = &mut self.cursor else {
            return;
        };
        pointer.pump_frames(screen);
    }

    fn cursor_events(&mut self) {
        let Some(pointer) = &mut self.cursor else {
            return;
        };
        pointer.drain();
    }
}

impl CursorStream {
    fn pump_frames(&mut self, screen: &mut Screen) {
        let generation = self.frames.frames().generation();
        if self.buffer.is_none() || self.generation != generation {
            let constraints = self.frames.frames().constraints();
            if constraints.is_empty() {
                return;
            }
            let Some(format) = constraints.best_shm() else {
                return;
            };
            self.buffer = None;
            match screen.shm_buffer(format, constraints.width, constraints.height) {
                Ok(buffer) => self.buffer = Some(buffer),
                Err(error) => {
                    eprintln!("rikka-portal-cast: cannot allocate cursor buffer: {error}");
                    return;
                }
            }
            self.generation = generation;
            self.is_fresh = false;
        }
        let Some(buffer) = &self.buffer else {
            return;
        };
        if self.frames.frames().is_capturing() {
            return;
        }
        if let Err(error) = self.frames.frames().capture(buffer) {
            eprintln!("rikka-portal-cast: cannot capture cursor: {error}");
        }
    }

    fn drain(&mut self) {
        for event in self.frames.frames().events() {
            match event {
                CaptureEvent::Ready(_) => self.is_fresh = true,
                CaptureEvent::ConstraintsChanged => self.buffer = None,
                _ => {}
            }
        }
    }

    fn position(&mut self) -> Option<CursorPosition<'_>> {
        let at = self.frames.cursor_position()?;
        let bitmap = match (self.is_fresh, &self.buffer) {
            (true, Some(buffer)) => buffer.pixels().map(|pixels| CursorBitmap {
                width: buffer.width(),
                height: buffer.height(),
                format: buffer.format(),
                pixels,
            }),
            _ => None,
        };
        self.is_fresh = false;
        Some(CursorPosition {
            x: at.x,
            y: at.y,
            hotspot_x: at.hotspot_x,
            hotspot_y: at.hotspot_y,
            bitmap,
        })
    }
}
