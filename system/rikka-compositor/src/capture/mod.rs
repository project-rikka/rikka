mod serve;
mod subject;
mod window;

use std::fmt;
use std::rc::Rc;

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Buffer, Size};
use rikka_compositor_output::{
    CaptureFormats, CaptureHost, CaptureRequest, CaptureSessionId, CaptureSource, Captures,
    Constraints, DmabufConstraints, DmabufFormat, SessionInfo,
};
use rikka_compositor_pixels::SHM_FORMATS;
use rikka_compositor_protocols::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use rikka_compositor_types::{Fourcc, Modifier, OutputId};
use rikka_renderer::{DmabufOutput, DmabufTarget, OffscreenTarget, Readback, Renderer};
use slotmap::SecondaryMap;

use crate::context::{CaptureCtx, CaptureRef, capture_ctx, capture_ref};
use crate::state::Compositor;
use crate::wayland::render_node;

#[derive(Default)]
pub(crate) struct Capture {
    fenced: Vec<serve::Fenced>,
    sessions: SecondaryMap<CaptureSessionId, Session>,
    waiting: Vec<CaptureRequest<WindowId>>,
    formats: Option<Rc<CaptureFormats>>,
}

#[derive(Default)]
struct Session {
    delivered: bool,
    target: Option<OffscreenTarget>,
    readback: Option<Readback>,
    reading: Option<serve::Reading>,
    destination: Option<DmabufOutput>,
    imported: Vec<(crate::render::DmabufKey, DmabufTarget)>,
}

impl CaptureHost for Compositor {
    type Toplevel = WindowId;

    fn captures(&mut self) -> &mut Captures<WindowId> {
        &mut self.captures
    }

    fn toplevel_source(&mut self, handle: &ExtForeignToplevelHandleV1) -> Option<WindowId> {
        capture_ref!(self).window_of_handle(handle)
    }

    fn capture_session_started(
        &mut self,
        session: CaptureSessionId,
        info: SessionInfo<WindowId>,
    ) -> Option<Constraints> {
        let output = capture_ref!(self).captured_output(info)?;
        let constraints = capture_ctx!(self).capture_constraints(output, info)?;
        self.capture.sessions.insert(session, Session::default());
        Some(constraints)
    }

    fn capture_requested(&mut self, request: CaptureRequest<WindowId>) {
        let output = capture_ref!(self).captured_output(request.info);
        self.capture.waiting.push(request);
        if let Some(output) = output {
            self.frames.request(output);
        }
    }
}

fn capture_formats(renderer: &Renderer) -> CaptureFormats {
    CaptureFormats {
        shm: [
            Fourcc::ARGB8888,
            Fourcc::XRGB8888,
            Fourcc::ABGR8888,
            Fourcc::XBGR8888,
        ]
        .into_iter()
        .filter(|format| SHM_FORMATS.contains(format))
        .collect(),
        dmabuf: dmabuf_constraints(renderer),
    }
}

fn dmabuf_constraints(renderer: &Renderer) -> Option<DmabufConstraints> {
    let (device, _) = render_node(renderer)?;
    let mut formats: Vec<(Fourcc, Vec<Modifier>)> = Vec::new();
    for format in renderer.gpu().dmabuf_render_formats() {
        let (fourcc, modifier) = (Fourcc(format.fourcc), Modifier(format.modifier));
        match formats.iter_mut().find(|(format, _)| *format == fourcc) {
            Some((_, modifiers)) => modifiers.push(modifier),
            None => formats.push((fourcc, vec![modifier])),
        }
    }
    (!formats.is_empty()).then(|| DmabufConstraints {
        device,
        formats: formats
            .into_iter()
            .map(|(fourcc, modifiers)| DmabufFormat {
                fourcc,
                modifiers: modifiers.into(),
            })
            .collect(),
    })
}

impl fmt::Debug for Capture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Capture")
            .field("sessions", &self.sessions.len())
            .field("waiting", &self.waiting.len())
            .finish()
    }
}

impl CaptureRef<'_> {
    fn captured_output(&self, info: SessionInfo<WindowId>) -> Option<OutputId> {
        match info.source {
            CaptureSource::Output(output) => Some(output),
            CaptureSource::Toplevel(window) => self.window_output(window),
        }
    }
}

impl CaptureCtx<'_> {
    fn capture_constraints(
        &mut self,
        output: OutputId,
        info: SessionInfo<WindowId>,
    ) -> Option<Constraints> {
        let size = self.capture_size(output, info)?;
        Some(Constraints {
            size,
            formats: self.capture_formats(),
        })
    }

    fn capture_size(
        &mut self,
        output: OutputId,
        info: SessionInfo<WindowId>,
    ) -> Option<Size<i32, Buffer>> {
        let scale = self.outputs.get(output)?.scale();
        match (info.cursor, info.source) {
            (Some(_), _) => {
                let image = self.cursor.image(scale);
                Some(Size::new(image.width as i32, image.height as i32))
            }
            (None, CaptureSource::Output(_)) => {
                let size = self.outputs.get(output)?.physical_size()?;
                Some(Size::new(size.w, size.h))
            }
            (None, CaptureSource::Toplevel(window)) => {
                window::buffer_size(capture_ref!(self).window_view(window)?)
            }
        }
    }

    fn capture_formats(&mut self) -> Rc<CaptureFormats> {
        let renderer = self.backend.renderer();
        Rc::clone(
            self.capture
                .formats
                .get_or_insert_with(|| Rc::new(capture_formats(renderer))),
        )
    }

    pub(crate) fn capture_constraints_changed(&mut self, output: OutputId) {
        let sessions: Vec<(CaptureSessionId, SessionInfo<WindowId>)> = self
            .captures
            .sessions()
            .filter(|(_, info)| capture_ref!(self).captured_output(*info) == Some(output))
            .collect();
        for (session, info) in sessions {
            if let Some(constraints) = self.capture_constraints(output, info) {
                self.captures.update_constraints(session, constraints);
            }
        }
    }

    pub(crate) fn forget_captures_of(&mut self, published: OutputId) {
        self.captures.stop_source(CaptureSource::Output(published));
        let renderer = self.backend.renderer();
        let captures = &self.captures;
        let scenes = &mut self.scenes;
        self.capture.sessions.retain(|session, _| {
            let alive = captures
                .session(session)
                .is_some_and(|info| info.source != CaptureSource::Output(published));
            if !alive {
                scenes.forget_capture(session, renderer);
            }
            alive
        });
        self.capture
            .waiting
            .retain(|request| request.info.source != CaptureSource::Output(published));
    }
}
