use std::fmt;
use std::io;
use std::sync::Arc;

use arc_swap::ArcSwap;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_input::{
    CursorPlacement, DeviceAccess, DeviceSettings, InputEvent, InputNotice, InputQueue,
    InputThread, Leds, PointerRestrictions, PointerShared,
};
use rikka_compositor_kms::{CrtcId, CursorControl, Session};
use rikka_compositor_reactor::{Interest, Reactor};
use rikka_compositor_types::OutputId;

use crate::error::{Error, Result};
use crate::input::{AbsoluteSource, Moved, PendingEvent, place as reach};
use crate::outputs::Outputs;
use crate::state::{Compositor, Parts};

use super::Kms;

#[derive(Debug, Default)]
struct CursorRoutes {
    crtcs: Vec<(OutputId, CursorControl, CrtcId)>,
}

pub(super) struct Pointer {
    thread: InputThread,
    shared: Arc<PointerShared>,
    routes: Arc<ArcSwap<CursorRoutes>>,
}

impl Pointer {
    pub(super) fn start(
        reactor: &Reactor<Compositor>,
        session: &Session,
        settings: DeviceSettings,
    ) -> Result<Self> {
        let notices = reactor
            .mailbox(|_: &mut Compositor, notice: InputNotice| {
                Kms::noticed(notice);
            })
            .map_err(Error::EventLoop)?;
        let queue = InputQueue::new().map_err(Error::EventLoop)?;
        let arriving = queue.clone();
        let mut arrived = Vec::new();
        reactor
            .watch(
                queue.fd(),
                Interest::READABLE,
                move |compositor: &mut Compositor, _| {
                    arriving.drain(&mut arrived);
                    Kms::input(compositor, &mut arrived);
                    arrived.clear();
                },
            )
            .map_err(Error::EventLoop)?;
        let opener = session.opener(reactor).map_err(Error::Kms)?;
        let closer = opener.clone();
        let access = DeviceAccess::new(
            move |path, _| opener.open(path).map_err(io::Error::other),
            move |fd| closer.close(fd),
        );
        let shared = Arc::new(PointerShared::default());
        let routes = Arc::new(ArcSwap::from_pointee(CursorRoutes::default()));
        let following = routes.clone();
        let thread = InputThread::builder(&queue, notices, shared.clone(), access)
            .seat(session.seat())
            .settings(settings)
            .cursor(move |placement: CursorPlacement| {
                place(
                    &following,
                    placement.target,
                    placement.local,
                    placement.scale,
                );
            })
            .spawn()
            .map_err(Error::Input)?;
        Ok(Self {
            thread,
            shared,
            routes,
        })
    }

    pub(super) fn configure(&self, settings: DeviceSettings) {
        if let Err(error) = self.thread.configure(settings) {
            tracing::warn!("cannot reconfigure input devices: {error}");
        }
    }

    pub(super) fn show_leds(&self, leds: Leds) {
        if let Err(error) = self.thread.set_leds(leds) {
            tracing::debug!("cannot set keyboard LEDs: {error}");
        }
    }

    pub(super) fn suspend(&mut self) {
        if let Err(error) = self.thread.suspend() {
            tracing::warn!("cannot suspend input devices: {error}");
        }
    }

    pub(super) fn resume(&self) {
        if let Err(error) = self.thread.resume() {
            tracing::warn!("cannot resume input devices: {error}");
        }
    }

    pub(super) fn publish(
        &self,
        outputs: &Outputs,
        restrictions: PointerRestrictions,
        hardware: bool,
        crtcs: Vec<(OutputId, CursorControl, CrtcId)>,
    ) {
        self.shared.publish(outputs.route(hardware, restrictions));
        self.routes.store(Arc::new(CursorRoutes { crtcs }));
    }

    pub(super) fn handle_events(&mut self, parts: &mut Parts<'_>, events: &mut Vec<InputEvent>) {
        for event in events.drain(..) {
            match event {
                InputEvent::PointerMotion {
                    time,
                    delta,
                    unaccelerated,
                    moved_to,
                    ..
                } => {
                    let placed = moved_to.is_some();
                    let to = match moved_to {
                        Some(to) => to,
                        None => self.warp_clamped(self.shared.position() + delta),
                    };
                    parts.input.pending.push(PendingEvent::Motion {
                        to,
                        moved: Some(Moved {
                            delta,
                            unaccelerated,
                        }),
                        placed,
                        time: time.as_monotonic(),
                    });
                }
                InputEvent::PointerMotionAbsolute {
                    device,
                    time,
                    position,
                } => {
                    let Some(to) = reach(parts.outputs, &parts.input.pending, device, position)
                    else {
                        continue;
                    };
                    let to = self.warp_clamped(to);
                    parts.input.pending.push_pointed(
                        AbsoluteSource::Device(device),
                        to,
                        time.as_monotonic(),
                    );
                }
                event => parts.input.queue_event(&event),
            }
        }
        parts.frames.input_arrived();
    }

    pub(super) fn warped(&self, parts: &mut Parts<'_>) {
        let to = parts.cursor.position();
        self.shared.warp(to);
        self.place_over(parts.outputs, to);
    }

    fn warp_clamped(&self, to: Point<f64, Logical>) -> Point<f64, Logical> {
        let to = self.shared.clamp(to);
        self.shared.warp(to);
        to
    }

    pub(super) fn place_over(&self, outputs: &Outputs, position: Point<f64, Logical>) {
        let Some(output) = outputs.containing(position) else {
            return;
        };
        if !output.has_cursor_plane() {
            return;
        }
        let Some(area) = output.area() else {
            return;
        };
        place(
            &self.routes,
            output.id(),
            position - area.loc.to_f64(),
            output.scale(),
        );
    }
}

fn place(routes: &ArcSwap<CursorRoutes>, target: OutputId, local: Point<f64, Logical>, scale: f64) {
    let routes = routes.load();
    let Some((_, control, crtc)) = routes.crtcs.iter().find(|(routed, _, _)| *routed == target)
    else {
        return;
    };
    let (x, y) = physical(local, scale);
    if let Err(error) = control.cursor_at(*crtc, x, y) {
        tracing::debug!("cannot move cursor plane: {error}");
    }
}

fn physical(local: Point<f64, Logical>, scale: f64) -> (i32, i32) {
    (nearest(local.x * scale), nearest(local.y * scale))
}

fn nearest(value: f64) -> i32 {
    (value + 0.5f64.copysign(value)) as i32
}

impl fmt::Debug for Pointer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Pointer")
            .field("position", &self.shared.position())
            .finish_non_exhaustive()
    }
}
