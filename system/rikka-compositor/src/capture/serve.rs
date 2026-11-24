use std::os::fd::{AsFd, OwnedFd};

use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Buffer, Point, Rect, Size, Transform};
use rikka_compositor_output::{
    CaptureFailure, CaptureFrameId, CaptureRequest, CaptureSessionId, CaptureSource, Constraints,
    CursorPlacement, FrameReady,
};
use rikka_compositor_pixels::write_rows;
use rikka_compositor_reactor::{Interest, Token};
use rikka_compositor_surface::{BufferContent, Dmabuf, Surfaces};
use rikka_compositor_types::OutputId;
use rikka_renderer::{DamageSource, DmabufFence, Renderer, Viewer};
use rikka_types::Rect as SceneRect;

use super::Session;
use super::subject::Subject;
use crate::agent::{Agent, Canvas};
use crate::context::{CaptureCtx, capture_ctx, capture_ref};
use crate::cursor::Cursor;
use crate::render::{Framing, Scenes, Stage, View, key_of, monotonic, with_client_spec};
use crate::state::Compositor;

pub(super) struct Reading {
    request: CaptureRequest<WindowId>,
    frame: u64,
    ready: FrameReady,
}

pub(super) struct Fenced {
    frame: CaptureFrameId,
    token: Token,
    _fence: OwnedFd,
    ready: FrameReady,
}

enum Served {
    Waiting(CaptureRequest<WindowId>),
    Done,
}

enum Drawing {
    Unchanged,
    Gone,
    Failed(CaptureFailure),
    Drawn(Delivery),
}

struct Delivery {
    ready: FrameReady,
    fence: Option<OwnedFd>,
    reads_back: bool,
}

struct Painted {
    damage: Vec<SceneRect>,
    fence: Option<OwnedFd>,
    reads_back: bool,
}

fn forget_composed(
    scenes: &mut Scenes,
    agent: &mut Agent,
    renderer: &mut Renderer,
    session: CaptureSessionId,
) {
    scenes.forget_capture(session, renderer);
    agent.forget(Canvas::Capture(session));
}

fn shown_output(request: &CaptureRequest<WindowId>, subject: &Subject) -> Option<OutputId> {
    let Framing::Screen { cursor: false } = subject.framing else {
        return None;
    };
    let screen = subject.view?.screen().size;
    (request.size.w == screen.w && request.size.h == screen.h)
        .then_some(subject.screen)
        .flatten()
}

fn render_capture(
    renderer: &mut Renderer,
    session: &mut Session,
    scene: &mut rikka_renderer::Scene,
    surfaces: &Surfaces,
    request: &CaptureRequest<WindowId>,
    viewer: Viewer,
) -> Result<Painted, CaptureFailure> {
    match surfaces.buffer_content(&request.buffer) {
        Some(BufferContent::Shm(_)) => onto_memory(renderer, session, scene, request.size, viewer),
        Some(BufferContent::Dmabuf(dmabuf)) => {
            onto_dmabuf(renderer, session, scene, dmabuf, viewer)
        }
        _ => Err(CaptureFailure::BufferConstraints),
    }
}

fn deliver(
    session: &mut Session,
    request: &CaptureRequest<WindowId>,
    drawn: Result<Painted, CaptureFailure>,
) -> Drawing {
    let painted = match drawn {
        Ok(painted) => painted,
        Err(reason) => return Drawing::Failed(reason),
    };
    let mut region = request.damage.clone();
    for rect in &painted.damage {
        region.add(Rect::covering(*rect));
    }
    if session.delivered && region.is_empty() {
        return Drawing::Unchanged;
    }
    session.delivered = true;
    Drawing::Drawn(Delivery {
        ready: FrameReady {
            presented: monotonic(),
            transform: Transform::Normal,
            damage: region,
        },
        fence: painted.fence,
        reads_back: painted.reads_back,
    })
}

fn onto_memory(
    renderer: &mut Renderer,
    session: &mut Session,
    scene: &mut rikka_renderer::Scene,
    size: Size<i32, Buffer>,
    viewer: Viewer,
) -> Result<Painted, CaptureFailure> {
    let (width, height) = (size.w.max(1).unsigned_abs(), size.h.max(1).unsigned_abs());
    if session
        .target
        .as_ref()
        .is_none_or(|target| target.width() != width || target.height() != height)
    {
        session.target = Some(renderer.create_target(width, height).map_err(|error| {
            tracing::warn!("cannot create capture target: {error}");
            CaptureFailure::Unknown
        })?);
    }
    let target = session.target.as_mut().ok_or(CaptureFailure::Unknown)?;
    target.set_viewer(viewer);
    let drawn = renderer.draw(scene, target).map_err(|error| {
        tracing::warn!("cannot render capture: {error}");
        CaptureFailure::Unknown
    })?;
    Ok(Painted {
        damage: drawn.damage.into_vec(),
        fence: None,
        reads_back: true,
    })
}

fn onto_dmabuf(
    renderer: &mut Renderer,
    session: &mut Session,
    scene: &mut rikka_renderer::Scene,
    dmabuf: &Dmabuf,
    viewer: Viewer,
) -> Result<Painted, CaptureFailure> {
    let key = key_of(dmabuf).ok_or(CaptureFailure::BufferConstraints)?;
    let index = match session
        .imported
        .iter()
        .position(|(imported_key, _)| *imported_key == key)
    {
        Some(index) => index,
        None => {
            let target =
                with_client_spec(dmabuf, |spec| renderer.import_dmabuf(spec)).map_err(|error| {
                    tracing::debug!("cannot import capture buffer: {error}");
                    CaptureFailure::BufferConstraints
                })?;
            session.imported.push((key, target));
            session.imported.len() - 1
        }
    };
    let destination = session
        .destination
        .get_or_insert_with(|| renderer.create_dmabuf_output(DamageSource::SinceLastFrame));
    destination.set_viewer(viewer);
    let (_, target) = &mut session.imported[index];
    let frame = renderer
        .draw(scene, destination.with_target(target))
        .map_err(|error| {
            tracing::warn!("cannot render capture: {error}");
            CaptureFailure::Unknown
        })?;
    Ok(Painted {
        damage: frame.output.damage.into_vec(),
        fence: frame.fence.map(DmabufFence::into_readable),
        reads_back: false,
    })
}

impl CaptureCtx<'_> {
    fn forget_stopped_captures(&mut self) {
        self.stop_departed_window_captures();
        let renderer = self.backend.renderer();
        let captures = &self.captures;
        let scenes = &mut self.scenes;
        let agent = &mut self.input.agent;
        self.capture.sessions.retain(|session, _| {
            let alive = captures.session(session).is_some();
            if !alive {
                forget_composed(scenes, agent, renderer, session);
            }
            alive
        });
        let sessions = &self.capture.sessions;
        self.capture.waiting.retain(|request| {
            sessions.contains_key(request.session) && captures.is_pending(request.frame)
        });
    }

    fn deliver_capture(&mut self, request: CaptureRequest<WindowId>, delivery: Delivery) {
        let Delivery {
            ready,
            fence,
            reads_back,
        } = delivery;
        if reads_back {
            return self.read_capture_back(request, ready);
        }
        let Some(fence) = fence else {
            self.captures.ready(request.frame, ready);
            return;
        };
        let frame = request.frame;
        let watched = self.core.reactor.watch(
            fence.as_fd(),
            Interest::READABLE,
            move |compositor: &mut Compositor, _| capture_ctx!(compositor).capture_fenced(frame),
        );
        match watched {
            Ok(token) => self.capture.fenced.push(Fenced {
                frame,
                token,
                _fence: fence,
                ready,
            }),
            Err(error) => {
                tracing::warn!("cannot watch capture fence: {error}");
                self.captures.fail(frame, CaptureFailure::Unknown);
            }
        }
    }

    fn read_capture_back(&mut self, request: CaptureRequest<WindowId>, ready: FrameReady) {
        let renderer = self.backend.renderer();
        let Some(session) = self.capture.sessions.get_mut(request.session) else {
            self.captures.fail(request.frame, CaptureFailure::Unknown);
            return;
        };
        let readback = session
            .readback
            .get_or_insert_with(|| renderer.create_readback());
        let Some(target) = session.target.as_mut() else {
            self.captures.fail(request.frame, CaptureFailure::Unknown);
            return;
        };
        if let Err(error) = renderer.start_readback(target, readback) {
            tracing::warn!("cannot read back capture: {error}");
            self.captures.fail(request.frame, CaptureFailure::Unknown);
            return;
        }
        let frame = renderer.submitted_frame();
        self.scenes.watch_frame(frame);
        session.reading = Some(Reading {
            request,
            frame,
            ready,
        });
    }

    fn measured_alike(&mut self, output: OutputId, request: &CaptureRequest<WindowId>) -> bool {
        let CaptureSource::Toplevel(window) = request.info.source else {
            return true;
        };
        let Some(size) = self.capture_size(output, request.info) else {
            self.captures.stop_source(CaptureSource::Toplevel(window));
            return false;
        };
        if self
            .captures
            .constraints(request.session)
            .is_some_and(|constraints| constraints.size == size)
        {
            return true;
        }
        let formats = self.capture_formats();
        self.captures
            .update_constraints(request.session, Constraints { size, formats });
        false
    }

    fn capture_fenced(&mut self, frame: CaptureFrameId) {
        let Some(index) = self
            .capture
            .fenced
            .iter()
            .position(|fenced| fenced.frame == frame)
        else {
            return;
        };
        let fenced = self.capture.fenced.swap_remove(index);
        self.core.reactor.remove(fenced.token);
        self.captures.ready(fenced.frame, fenced.ready);
    }

    pub(crate) fn capture_read(&mut self, finished: u64) {
        let renderer = self.backend.renderer();
        let surfaces = &mut self.surfaces;
        let captures = &mut self.captures;
        for session in self.capture.sessions.values_mut() {
            if session
                .reading
                .as_ref()
                .is_none_or(|reading| reading.frame > finished)
            {
                continue;
            }
            let Some(readback) = session.readback.as_mut() else {
                continue;
            };
            if !renderer.poll_readback(readback) {
                continue;
            }
            let Some(reading) = session.reading.take() else {
                continue;
            };
            let content = match surfaces.buffer_content(&reading.request.buffer) {
                Some(BufferContent::Shm(shm)) => {
                    let (format, stride, size) = (shm.format(), shm.stride(), shm.size());
                    let mut shm = shm;
                    shm.with_pixels_mut(|pixels| {
                        write_rows(
                            pixels,
                            format,
                            stride,
                            (
                                size.w.max(0).unsigned_abs() as usize,
                                size.h.max(0).unsigned_abs() as usize,
                            ),
                            readback.pixels(),
                            [Rect::from_size(size)],
                        )
                    })
                    .unwrap_or(false)
                }
                _ => false,
            };
            match content {
                true => captures.ready(reading.request.frame, reading.ready),
                false => captures.fail(reading.request.frame, CaptureFailure::BufferConstraints),
            };
        }
    }

    pub(crate) fn place_capture_cursors(&mut self) {
        let placements: Vec<_> = self
            .captures
            .cursor_sessions()
            .map(|(cursor, source, _)| (cursor, source))
            .collect();
        for (cursor, source) in placements {
            let view = match source {
                CaptureSource::Output(output) => {
                    self.outputs.get(output).and_then(View::from_output)
                }
                CaptureSource::Toplevel(window) => capture_ref!(self).window_view(window),
            };
            let placement = view.and_then(|view| self.cursor.cursor_in(view));
            self.captures.place_cursor(cursor, placement);
        }
    }

    pub(crate) fn frame_finished(&mut self, output: OutputId) {
        self.serve_captures(output);
    }

    fn serve_captures(&mut self, output: OutputId) {
        self.forget_stopped_captures();
        let requests = std::mem::take(&mut self.capture.waiting);
        for request in requests {
            let busy = self
                .capture
                .sessions
                .get(request.session)
                .is_some_and(|session| session.reading.is_some());
            if busy || capture_ref!(self).captured_output(request.info) != Some(output) {
                self.capture.waiting.push(request);
                continue;
            }
            if let Served::Waiting(request) = self.serve(output, request) {
                self.capture.waiting.push(request);
            }
        }
    }

    fn serve(&mut self, output: OutputId, request: CaptureRequest<WindowId>) -> Served {
        if !self.measured_alike(output, &request) {
            return Served::Done;
        }
        let Some(subject) = capture_ref!(self).capture_subject(output, request.info) else {
            return Served::Waiting(request);
        };
        match self.draw_capture(&request, subject) {
            Drawing::Unchanged => Served::Waiting(request),
            Drawing::Gone => Served::Done,
            Drawing::Failed(reason) => {
                self.captures.fail(request.frame, reason);
                Served::Done
            }
            Drawing::Drawn(delivery) => {
                self.deliver_capture(request, delivery);
                Served::Done
            }
        }
    }

    fn draw_capture(&mut self, request: &CaptureRequest<WindowId>, subject: Subject) -> Drawing {
        match shown_output(request, &subject) {
            Some(output) if self.scenes.composed(output).is_some() => {
                self.draw_from_output(request, output)
            }
            _ => self.draw_composed(request, subject),
        }
    }

    fn draw_from_output(
        &mut self,
        request: &CaptureRequest<WindowId>,
        output: OutputId,
    ) -> Drawing {
        let CaptureCtx {
            backend,
            scenes,
            surfaces,
            capture,
            input,
            ..
        } = self;
        let renderer = backend.renderer();
        let Some(session) = capture
            .sessions
            .entry(request.session)
            .map(|entry| entry.or_default())
        else {
            return Drawing::Gone;
        };
        forget_composed(scenes, &mut input.agent, renderer, request.session);
        let Some(scene) = scenes.scene(output) else {
            return Drawing::Gone;
        };
        let drawn = render_capture(renderer, session, scene, surfaces, request, Viewer::Capture);
        let delivery = deliver(session, request, drawn);
        if let Drawing::Drawn(_) = &delivery {
            scenes.output_captured(output, renderer.submitted_frame());
        }
        delivery
    }

    fn draw_composed(&mut self, request: &CaptureRequest<WindowId>, subject: Subject) -> Drawing {
        let Subject {
            view,
            framing,
            composed,
            icon,
            screen,
        } = subject;
        let CaptureCtx {
            backend,
            scenes,
            cursor,
            surfaces,
            capture,
            windows,
            input,
            outputs,
            ..
        } = self;
        let renderer = backend.renderer();
        let Some(session) = capture
            .sessions
            .entry(request.session)
            .map(|entry| entry.or_default())
        else {
            return Drawing::Gone;
        };
        let scene = scenes.compose_capture(
            request.session,
            view,
            framing,
            Stage {
                cursor,
                surfaces,
                drawn: composed.drawn,
                staging: composed.staging,
                icon,
                corners: false,
                locked: windows.desktop.locked(),
            },
            renderer,
        );
        if let (Framing::Screen { .. }, Some(output)) =
            (framing, screen.and_then(|screen| outputs.get(screen)))
        {
            input
                .agent
                .draw(scene, Canvas::Capture(request.session), output);
        }
        if session.delivered && !scene.has_damage() {
            return Drawing::Unchanged;
        }
        let drawn = render_capture(renderer, session, scene, surfaces, request, Viewer::Screen);
        let delivery = deliver(session, request, drawn);
        if let Drawing::Drawn(_) = &delivery {
            scenes.capture_drawn(
                request.session,
                renderer.submitted_frame(),
                surfaces,
                monotonic(),
            );
        }
        delivery
    }
}

impl Cursor {
    fn cursor_in(&mut self, view: View) -> Option<CursorPlacement> {
        let position = self.position();
        if self.hidden() || !view.area.to_f64().contains(position) {
            return None;
        }
        let scale = view.scale;
        let image = self.image(scale);
        let local = position - view.area.loc.to_f64();
        Some(CursorPlacement {
            position: Point::new(
                (local.x * scale).round() as i32,
                (local.y * scale).round() as i32,
            ),
            hotspot: Point::new(image.hotspot.0, image.hotspot.1),
        })
    }
}
