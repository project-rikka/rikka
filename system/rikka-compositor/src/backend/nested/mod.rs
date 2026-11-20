mod host;
mod input;
mod screens;
mod shortcuts;
mod window;

use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::fmt;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_compositor_geometry::{Point, Size, Transform};
use rikka_compositor_input::DeviceSettings;
use rikka_compositor_reactor::{Interest, Reactor, Timestamp, Token};
use rikka_compositor_surface::{Presentation, PresentationKind, Refresh};
use rikka_compositor_types::OutputId;
use rikka_present::Presenter;
use rikka_renderer::{
    Acquired, Buffering, FrameStart, Present, Recorded, Renderer, Scene, SurfaceConfig,
};
use rikka_seat::Seats;
use rikka_types::CursorShape;
use rikka_wayland::{Display, PresentationFeedback};
use rikka_wayland_client::{Connection, Queue};

use self::host::{Heard, Host};
use self::input::{Arrived, HostInput, Shown};
use self::shortcuts::HostShortcuts;
use self::window::{Shape, Window};
use super::Backend;
use crate::error::{Error, Result};
use crate::input::AbsoluteSource;
use crate::outputs::{FALLBACK_REFRESH, Head, Mode, OutputConfig, Outputs, interval};
use crate::programs::COMPOSITOR;
use crate::render::{Presenting, Snapshot, monotonic};
use crate::session::NiceCapability;
use crate::state::{Compositor, Parts};

const HEAD: &str = "nested-0";

const CURSOR_SCALE: u32 = 1;

pub(crate) struct Nested {
    reactor: Reactor<Compositor>,
    watch: Token,
    due: Option<Token>,
    connection: Connection,
    queue: Queue<Host>,
    host: Host,
    _shortcuts: Option<HostShortcuts>,
    timing: Option<rikka_wayland::Presentation>,
    shown: Arc<Mutex<Vec<PresentationFeedback>>>,
    renderer: Renderer,
    presenter: Presenter,
    output: OutputId,
    shape: Shape,
    refresh: u32,
    transform: Transform,
    drawn: (u32, u32),
    input: HostInput,
    wanted: bool,
    awaiting: bool,
    landing: VecDeque<(u64, Presenting)>,
    sequence: u64,
    declared: bool,
}

enum Drawn {
    Presentable(Recorded),
    Unchanged,
    Deferred,
    Busy,
}

impl Nested {
    pub(crate) fn start(
        reactor: &Reactor<Compositor>,
        outputs: &mut Outputs,
        nice: NiceCapability,
        socket: &str,
    ) -> Result<Self> {
        let display = Display::from_env().map_err(Error::Host)?;
        let connection = display.connection().clone();
        let mut queue = connection.queue::<Host>();
        Host::listen(&mut queue);
        let route = queue.route();
        let window = Window::open(&display, &route, &format!("Rikka — {socket}"), COMPOSITOR)
            .map_err(Error::Host)?;
        let mut seats = Seats::builder()
            .gestures()
            .relative_motion()
            .bind(&display, &route);
        seats.set_relative_motion(true);
        display.announce_globals(&route);
        let mut host = Host::new(seats, window);
        let lost = |error: rikka_wayland_client::Error| Error::Host(error.into());
        connection.flush().map_err(lost)?;
        while !host.window.is_configured() {
            queue.blocking_dispatch(&mut host).map_err(lost)?;
        }
        queue.roundtrip(&mut host).map_err(lost)?;

        let renderer = Renderer::new(
            &rikka_gpu::Gpu::builder()
                .app_name(COMPOSITOR)
                .latency_critical()
                .upload_queue()
                .build()
                .map_err(Error::Renderer)?,
        )
        .map_err(Error::Renderer)?;
        nice.leave_state_thread();
        let mut renderer = renderer;
        let presenter = Presenter::new(
            &rikka_wayland::Host::read_by_owner(display.clone()),
            host.window.surface(),
            &mut renderer,
            SurfaceConfig {
                buffering: Buffering::Triple,
                ..SurfaceConfig::default()
            },
        )
        .map_err(Error::Present)?;
        let shown = Arc::new(Mutex::new(Vec::new()));
        let landed = Arc::clone(&shown);
        let timing = rikka_wayland::Presentation::new(&display, move |feedback| {
            landed.lock().push(feedback);
        })
        .inspect_err(|error| tracing::info!("host reports no presentation times: {error}"))
        .ok();
        let shortcuts = HostShortcuts::hold(&display, host.window.surface());

        let shape = host.window.shape(&host.screens);
        let refresh = host
            .screens
            .refresh_of(host.window.outputs())
            .unwrap_or(FALLBACK_REFRESH);
        let output = outputs.connect(head(shape, refresh));
        let watch = reactor
            .watch(connection.as_fd(), Interest::READABLE, |compositor, _| {
                Nested::pump(compositor);
            })
            .map_err(Error::EventLoop)?;
        reactor.idle(Nested::pump);
        let mut nested = Self {
            reactor: reactor.clone(),
            watch,
            due: None,
            connection,
            queue,
            host,
            _shortcuts: shortcuts,
            timing,
            shown,
            renderer,
            presenter,
            output,
            shape,
            refresh,
            transform: Transform::Normal,
            drawn: shape.logical,
            input: HostInput::default(),
            wanted: false,
            awaiting: false,
            landing: VecDeque::new(),
            sequence: 0,
            declared: true,
        };
        nested.fit();
        tracing::info!(
            width = shape.pixels.width,
            height = shape.pixels.height,
            scale = shape.scale,
            "host window configured"
        );
        Ok(nested)
    }

    fn pump(compositor: &mut Compositor) {
        let (closed, output, reshaped) = {
            let Some((nested, mut parts)) = compositor.backend::<Nested>() else {
                return;
            };
            let heard = nested.hear(&mut parts);
            (heard.closed, nested.output, heard.reshaped)
        };
        let described = reshaped.head.is_some();
        if let Some(head) = reshaped.head {
            compositor.outputs.describe(output, head);
        }
        if reshaped.declare {
            compositor.displays_detected();
        } else if described {
            compositor.displays_resized();
        }
        if closed {
            tracing::info!("host window closed");
            compositor.stop();
        }
    }

    fn frame(compositor: &mut Compositor) {
        let Some((nested, mut parts)) = compositor.backend::<Nested>() else {
            return;
        };
        nested.due = None;
        if nested.wanted && !nested.awaiting {
            nested.draw(&mut parts);
        }
    }

    fn hear(&mut self, parts: &mut Parts<'_>) -> Hearing {
        let read = self
            .connection
            .read()
            .and_then(|_| self.queue.dispatch_pending(&mut self.host));
        if let Err(error) = read {
            tracing::error!("host compositor connection lost: {error}");
            return Hearing {
                closed: true,
                reshaped: Reshaped::default(),
            };
        }
        Seats::flush(&mut self.host);
        self.forward_input(parts);
        self.host.seats.show_cursor(
            self.host.window.surface(),
            CursorShape::Hidden,
            CURSOR_SCALE,
        );
        let reshaped = self.reshape(parts);
        let shown = std::mem::take(&mut *self.shown.lock());
        for feedback in shown {
            self.landed(parts, feedback);
        }
        if std::mem::take(&mut self.host.frame_due) {
            self.awaiting = false;
            if self.wanted && reshaped.head.is_none() {
                self.draw(parts);
            }
        }
        self.flush();
        Hearing {
            closed: self.host.closed,
            reshaped,
        }
    }

    fn forward_input(&mut self, parts: &mut Parts<'_>) {
        if self.host.heard.is_empty() {
            return;
        }
        let output = parts.outputs.get(self.output);
        let shown = Shown {
            origin: output
                .and_then(|output| output.area())
                .map_or(Point::zero(), |area| area.loc.to_f64()),
            scale: output.map_or(self.shape.scale, |output| output.scale()),
            transform: self.transform,
            window: Size::new(
                f64::from(self.shape.pixels.width),
                f64::from(self.shape.pixels.height),
            ),
            host_scale: self.shape.scale,
        };
        let time = monotonic();
        let pending = &mut parts.input.pending;
        let mut arrived = false;
        let mut emit = |event: Arrived| {
            arrived = true;
            match event {
                Arrived::Pointed(to) => pending.push_pointed(AbsoluteSource::Host, to, time),
                Arrived::Event(event) => pending.push(event),
            }
        };
        for heard in self.host.heard.drain(..) {
            match heard {
                Heard::Input(event) => self.input.translate(event, shown, time, &mut emit),
                Heard::Unfocused => self.input.unfocused(time, &mut emit),
            }
        }
        self.input.settle(time, &mut emit);
        if arrived {
            parts.frames.input_arrived();
        }
    }

    fn reshape(&mut self, parts: &mut Parts<'_>) -> Reshaped {
        if !std::mem::take(&mut self.host.reshaped) {
            return Reshaped::default();
        }
        let shape = self.host.window.shape(&self.host.screens);
        let refresh = self
            .host
            .screens
            .refresh_of(self.host.window.outputs())
            .unwrap_or(FALLBACK_REFRESH);
        let before = std::mem::replace(&mut self.shape, shape);
        let described =
            (before.pixels, before.scale, self.refresh) != (shape.pixels, shape.scale, refresh);
        self.refresh = refresh;
        if before != shape && before.pixels == shape.pixels {
            self.fit();
            parts.frames.request(self.output);
        }
        if described {
            self.declared = false;
        }
        let declare = !shape.resizing && !self.declared;
        self.declared |= declare;
        Reshaped {
            head: described.then(|| head(shape, refresh)),
            declare,
        }
    }

    fn fit(&mut self) {
        let Shape {
            logical,
            pixels,
            scale,
            resizing,
        } = self.shape;
        self.presenter
            .resize((pixels.width, pixels.height), scale, resizing);
        self.drawn = logical;
    }

    fn draw(&mut self, parts: &mut Parts<'_>) {
        self.wanted = false;
        let Some(scene) = parts.compose(self.output, &mut self.renderer) else {
            return;
        };
        let drawn = record(&mut self.renderer, scene, &mut self.presenter)
            .inspect_err(|error| tracing::warn!("cannot draw nested window: {error}"));
        let frame = self.renderer.submitted_frame();
        let recorded = match drawn {
            Ok(Drawn::Presentable(recorded)) => recorded,
            Ok(Drawn::Deferred) => {
                parts.scenes.damage_whole(self.output);
                parts.frames.request(self.output);
                return self.undrawn(parts, frame);
            }
            Ok(Drawn::Busy) => {
                self.wanted = true;
                self.retry();
                return self.undrawn(parts, frame);
            }
            Ok(Drawn::Unchanged) | Err(_) => return self.undrawn(parts, frame),
        };
        let key = self.sequence + 1;
        self.host.window.frame(self.drawn);
        if let Some(timing) = &self.timing {
            timing.feedback(self.host.window.surface(), key);
        }
        if let Err(error) = self.presenter.present(recorded) {
            tracing::warn!("cannot present nested window: {error}");
            return self.undrawn(parts, frame);
        }
        tracing::trace!(output = ?self.output, frame, "frame presented to host");
        self.sequence = key;
        self.awaiting = true;
        parts.frames_done(self.output, frame);
        let presenting = parts.feedback(self.output, None);
        match self.timing {
            Some(_) => self.landing.push_back((key, presenting)),
            None => {
                let presentation = self.presentation(key, monotonic(), None);
                parts.presented(self.output, presenting, &presentation);
            }
        }
    }

    fn undrawn(&mut self, parts: &mut Parts<'_>, frame: u64) {
        let presentation = self.presentation(self.sequence, monotonic(), None);
        parts.composited(self.output, frame, false, &presentation);
    }

    fn landed(&mut self, parts: &mut Parts<'_>, feedback: PresentationFeedback) {
        while self
            .landing
            .front()
            .is_some_and(|(key, _)| *key <= feedback.key)
        {
            let Some((key, presenting)) = self.landing.pop_front() else {
                return;
            };
            let shown = feedback.presented.filter(|_| key == feedback.key);
            tracing::trace!(output = ?self.output, key, shown = shown.is_some(), "host answered frame");
            match shown {
                Some(shown) => {
                    let presentation = Presentation {
                        flags: PresentationKind::VSYNC,
                        ..self.presentation(
                            key,
                            Timestamp::from_instant(shown.timestamp).as_monotonic(),
                            shown.refresh,
                        )
                    };
                    parts.presented(self.output, presenting, &presentation);
                }
                None => {
                    drop(presenting);
                    parts.refreshed(self.output, monotonic());
                }
            }
        }
    }

    fn presentation(
        &self,
        sequence: u64,
        time: Duration,
        refresh: Option<Duration>,
    ) -> Presentation {
        Presentation {
            time,
            refresh: Refresh::Fixed(refresh.unwrap_or_else(|| self.interval())),
            sequence,
            flags: PresentationKind::empty(),
        }
    }

    fn interval(&self) -> Duration {
        interval(self.refresh)
    }

    fn retry(&mut self) {
        if self.due.is_none() {
            self.due = Some(self.reactor.after(self.interval(), Nested::frame));
        }
    }

    fn flush(&self) {
        if let Err(error) = self.connection.flush() {
            tracing::debug!("cannot flush host connection: {error}");
        }
    }
}

struct Hearing {
    closed: bool,
    reshaped: Reshaped,
}

#[derive(Default)]
struct Reshaped {
    head: Option<Head>,
    declare: bool,
}

fn record(
    renderer: &mut Renderer,
    scene: &mut Scene,
    presenter: &mut Presenter,
) -> rikka_renderer::Result<Drawn> {
    match presenter.start(renderer, scene)? {
        FrameStart::Idle => return Ok(Drawn::Unchanged),
        FrameStart::Deferred => return Ok(Drawn::Deferred),
        FrameStart::Draw => {}
    }
    let Acquired::Image(image) = presenter.acquire_image(Duration::ZERO)? else {
        return Ok(Drawn::Busy);
    };
    let recorded = presenter.record_image(renderer, scene, image)?;
    Ok(match recorded.is_presentable() {
        true => Drawn::Presentable(recorded),
        false if recorded.into_output().needs_full_redraw => Drawn::Deferred,
        false => Drawn::Unchanged,
    })
}

fn head(shape: Shape, refresh: u32) -> Head {
    Head {
        name: HEAD.to_owned(),
        make: "Rikka".to_owned(),
        model: "Nested".to_owned(),
        serial: None,
        size_mm: (0, 0),
        modes: vec![Mode {
            width: shape.pixels.width,
            height: shape.pixels.height,
            refresh,
        }],
        scale: shape.scale,
        hdr: None,
    }
}

impl Backend for Nested {
    fn configure(&mut self, parts: &mut Parts<'_>, configs: &[(OutputId, OutputConfig)]) -> bool {
        let Some(offered) = parts
            .outputs
            .get(self.output)
            .map(|output| output.head().modes.clone())
        else {
            return false;
        };
        if configs.iter().any(|(output, config)| {
            *output != self.output || !config.enabled || !offered.contains(&config.mode)
        }) {
            return false;
        }
        for (output, config) in configs {
            self.transform = config.transform;
            self.presenter.set_transform(config.transform);
            self.fit();
            parts.outputs.apply(
                *output,
                OutputConfig {
                    hdr: false,
                    ..*config
                },
            );
            parts.frames.request(*output);
        }
        true
    }

    fn schedule(&mut self, output: OutputId) {
        if output != self.output {
            return;
        }
        self.wanted = true;
        if !self.awaiting && self.due.is_none() {
            self.due = Some(self.reactor.timer(Instant::now(), Nested::frame));
        }
    }

    fn configure_devices(&mut self, _: DeviceSettings) {}

    fn cursor_changed(&mut self, parts: &mut Parts<'_>) {
        parts.frames.request(self.output);
    }

    fn switches_vt(&self) -> bool {
        false
    }

    fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    fn snapshot(&mut self, _: OutputId) -> Option<Snapshot> {
        None
    }
}

impl Drop for Nested {
    fn drop(&mut self) {
        self.reactor.remove(self.watch);
        if let Some(due) = self.due.take() {
            self.reactor.remove(due);
        }
        self.flush();
    }
}

impl fmt::Debug for Nested {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Nested")
            .field("output", &self.output)
            .field("shape", &self.shape)
            .finish_non_exhaustive()
    }
}
