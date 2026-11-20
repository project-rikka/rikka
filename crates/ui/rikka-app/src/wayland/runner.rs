use std::sync::Arc;
use std::time::Instant;

use rikka_instance::LaunchRequest;
use rikka_present::Presenter;
use rikka_renderer::{Renderer, SurfaceConfig};
use rikka_seat::{PadEvent, SeatEvent, SeatHost, Seats};
use rikka_theme::Theme;
use rikka_types::{Key, PixelSize, Size, Stylus, WindowIdentifier};
use rikka_ui::{Event, PadInput};
use rikka_ui_host::VideoPlayback;
use rikka_wayland_client::Queue;

use crate::device::Device;
use crate::error::{Error, Result};
use crate::instance::Policy;
use crate::tasks::{LoopEvent, LoopProxy, LoopSink, WindowId};
use crate::wayland::client::Client;
use crate::wayland::drag::Dragging;
use crate::wayland::hosting::{self, Hosting, Windows};
use crate::wayland::open;
use crate::wayland::session::Session;
use crate::wayland::window_state::WindowState;
use crate::window::{self, Opening};

pub(crate) struct Runner {
    app_name: String,
    device: Device,
    theme: Theme,
    pending: Vec<Opening>,
    hosting: Hosting,
    session: Session,
    dragging: Dragging,
    policy: Option<Policy>,
    proxy: LoopProxy,
    pub(crate) client: Client,
    next_id: WindowId,
    touched: Vec<WindowId>,
    settling: Vec<WindowId>,
    pub(crate) screens_moved: bool,
    travelling: bool,
    exiting: bool,
    error: Option<Error>,
}

pub(crate) struct RunnerConfig {
    pub(crate) app_name: String,
    pub(crate) device: Device,
    pub(crate) videos: Option<Box<dyn VideoPlayback>>,
    pub(crate) theme: Theme,
    pub(crate) windows: Vec<Opening>,
    pub(crate) policy: Option<Policy>,
    pub(crate) arriving: Option<String>,
    pub(crate) proxy: LoopProxy,
}

impl Runner {
    pub(crate) fn new(config: RunnerConfig, client: Client) -> Runner {
        let RunnerConfig {
            app_name,
            device,
            videos,
            theme,
            mut windows,
            policy,
            arriving,
            proxy,
        } = config;
        let mut hosting = Hosting::new(LoopSink::new(proxy.clone()));
        let waking = proxy.clone();
        hosting.wakes_with(Arc::new(move || waking.wake()));
        if let Some(videos) = videos {
            hosting.plays_video(videos);
        }
        if let Some(first) = windows.first_mut() {
            first.look.token = arriving;
        }
        Runner {
            app_name,
            device,
            theme,
            pending: windows,
            hosting,
            session: Session::default(),
            dragging: Dragging::default(),
            policy,
            proxy,
            client,
            next_id: WindowId::from(1),
            touched: Vec::new(),
            settling: Vec::new(),
            screens_moved: false,
            travelling: false,
            exiting: false,
            error: None,
        }
    }

    pub(crate) fn start(&mut self, queue: &mut Queue<Runner>) -> Result<()> {
        queue.set_waker(self.proxy.waker());
        queue.roundtrip(self).map_err(rikka_wayland::Error::from)?;
        queue.roundtrip(self).map_err(rikka_wayland::Error::from)?;
        self.client.host.set_serials(self.client.seats.serials());
        self.session = Session::open(&self.client.host, &self.proxy, &self.app_name);
        window::accept_window_requests();
        self.open_pending()?;
        if self.hosting.is_empty() {
            self.exiting = true;
        }
        Ok(())
    }

    pub(crate) fn exiting(&self) -> bool {
        self.exiting
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.release();
        match self.error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn release(&mut self) {
        let Self {
            hosting,
            dragging,
            session,
            ..
        } = self;
        dragging.release();
        hosting.shutdown();
        session.release();
    }

    pub(crate) fn fail(&mut self, error: Error) {
        tracing::error!("fatal error: {error}");
        if self.error.is_none() {
            self.error = Some(error);
        }
        self.exiting = true;
    }

    pub(crate) fn window_mut(&mut self, id: WindowId) -> Option<&mut WindowState> {
        self.hosting.get_mut(&id).map(|hosted| &mut hosted.state)
    }

    pub(crate) fn identify(&mut self, id: WindowId, identifier: WindowIdentifier) {
        if let Some(hosted) = self.hosting.get_mut(&id) {
            hosted.ui.set_window_identifier(Some(identifier));
        }
    }

    pub(crate) fn frame_answered(&mut self, id: WindowId) {
        if let Some(hosted) = self.hosting.get_mut(&id) {
            hosted.frame_answered();
        }
    }

    pub(crate) fn forget_output(&mut self, output: &rikka_wayland_client::wl_output::WlOutput) {
        for hosted in self.hosting.values_mut() {
            hosted.state.outputs.retain(|entered| entered != output);
        }
        self.screens_moved = true;
    }

    pub(crate) fn touch(&mut self, id: WindowId) {
        if !self.touched.contains(&id) {
            self.touched.push(id);
        }
    }

    fn open_pending(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let app_name = self.app_name.clone();
        let device = self.device;
        self.hosting
            .renderer_or_try_init(|| {
                let gpu = device
                    .ask(rikka_gpu::Gpu::builder().app_name(app_name))
                    .build()?;
                Ok::<_, Error>(Renderer::new(&gpu)?)
            })
            .map(drop)?;
        let Self {
            hosting,
            session,
            dragging,
            theme,
            proxy,
            app_name,
            pending,
            client,
            next_id,
            ..
        } = self;
        let mut closing = Vec::new();
        let mut windows = Windows::new(hosting, session, client, &mut closing);
        for opening in std::mem::take(pending) {
            let id = *next_id;
            *next_id = id.after();
            open::open(&mut windows, id, proxy, app_name.as_str(), theme, opening);
            dragging.opened(&mut windows, id);
        }
        self.publish_refresh();
        Ok(())
    }

    fn open_requested(&mut self, openings: Vec<Opening>) {
        if openings.is_empty() {
            return;
        }
        self.pending.extend(openings);
        if let Err(error) = self.open_pending() {
            self.pending.clear();
            tracing::error!("failed to open window: {error}");
        }
    }

    fn windows(&mut self, act: impl FnOnce(&mut Windows<'_>)) {
        let Self {
            hosting,
            session,
            client,
            ..
        } = self;
        let mut closing = Vec::new();
        act(&mut Windows::new(hosting, session, client, &mut closing));
        for id in closing {
            self.close(id);
        }
    }

    fn dragging(&mut self, act: impl FnOnce(&mut Dragging, &mut Windows<'_>)) {
        let Self {
            hosting,
            session,
            dragging,
            client,
            ..
        } = self;
        let mut closing = Vec::new();
        act(
            dragging,
            &mut Windows::new(hosting, session, client, &mut closing),
        );
        for id in closing {
            self.close(id);
        }
    }

    fn service(&mut self, id: WindowId) {
        self.dragging(|dragging, windows| {
            dragging.begin_from(windows, id);
            windows.service(id);
        });
    }

    fn presented(&mut self, feedback: rikka_wayland::PresentationFeedback) {
        let (Some(hosted), Some(time)) = (
            self.hosting.get_mut(&WindowId::from(feedback.key)),
            feedback.presented,
        ) else {
            return;
        };
        hosted.presented(time.timestamp, time.refresh);
    }

    fn retheme(&mut self, theme: Theme) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        let glaze = self.theme.translucency();
        let blurs = glaze.blur_radius().is_some();
        let Self {
            hosting,
            session,
            theme,
            ..
        } = self;
        for hosted in hosting.values_mut() {
            hosted.ui.set_theme(theme.clone());
            let transparent = hosted.state.needs_alpha || glaze.is_translucent();
            hosted.state.set_transparent(&mut hosted.ui, transparent);
            hosted.adjust(move |surface| surface.set_transparent(transparent));
            let surface = hosted.state.toplevel.surface().clone();
            hosted
                .state
                .glazing
                .set_blur(blurs, || session.blurred(&surface));
        }
    }

    fn relabel(&mut self) {
        let mut heard = Vec::new();
        for hosted in self.hosting.values_mut() {
            if let Some(hook) = hosted.state.relabel(&mut hosted.ui) {
                hosted.ui.start(hook);
                heard.push(hosted.state.id);
            }
        }
        for id in heard {
            self.touch(id);
        }
    }

    fn opened(&mut self, open: LaunchRequest) {
        if matches!(self.policy, Some(Policy::Single)) {
            self.reactivate(open.token());
            self.reopened(open);
            return;
        }
        let Some(Policy::Windows(make)) = &self.policy else {
            return;
        };
        let token = open.token().map(str::to_owned);
        let mut windows = make(open);
        if let Some(first) = windows.first_mut() {
            first.look.token = token;
        }
        self.open_requested(windows);
    }

    fn reopened(&mut self, open: LaunchRequest) {
        let reached = self.hosting.values_mut().find_map(|hosted| {
            let handler = hosted.state.opened(open.clone())?;
            hosted.ui.start(handler);
            Some(hosted.state.id)
        });
        if let Some(id) = reached {
            self.touch(id);
        }
    }

    fn reactivate(&self, token: Option<&str>) {
        let shown = self
            .hosting
            .iter()
            .next()
            .map(|(_, hosted)| hosted.state.toplevel.surface());
        self.session.reactivate(shown, token, &self.app_name);
    }

    fn close(&mut self, id: WindowId) {
        if let Some(hosted) = self.hosting.get(&id) {
            hosted.state.alive.close();
        }
        self.dragging.window_gone(&self.session, id);
        drop(self.hosting.remove(&id));
        if self.hosting.is_empty() {
            self.exiting = true;
        }
    }

    pub(crate) fn user_event(&mut self, event: LoopEvent) {
        match event {
            LoopEvent::Opened(open) => self.opened(*open),
            LoopEvent::Message(message) => {
                self.hosting.deliver(&message.window, message.inbound);
                self.touch(message.window);
            }
            LoopEvent::Theme(theme) => self.retheme(*theme),
            LoopEvent::Language => self.relabel(),
            LoopEvent::Drag(event) => self.dragging(|dragging, windows| {
                dragging.handle_event(windows, *event);
            }),
            LoopEvent::Presented(presented) => self.presented(presented),
        }
    }

    pub(crate) fn turn(&mut self) -> Option<Instant> {
        Seats::flush(self);
        Seats::repeat(self, Instant::now());
        self.settle_windows();
        let mut touched = std::mem::take(&mut self.touched);
        for id in touched.drain(..) {
            self.service(id);
        }
        touched.append(&mut self.touched);
        self.touched = touched;
        self.dragging(Dragging::settle_within);
        let openings = window::take_requested_windows();
        self.open_requested(openings);
        let outcome = self.hosting.frame(|drawing| {
            drawing
                .state
                .glazing
                .set_regions(drawing.ui.glass(), drawing.ui.opaque());
        });
        for hosted in self.hosting.values_mut() {
            if !hosted.is_painting() {
                hosted.state.settle_unchanged();
            }
        }
        let carried = self.dragging.paint_ghost(&mut self.hosting);
        self.windows(|windows| windows.service_all());
        self.follow_seats();
        let hinted = outcome.wake.delay().map(|delay| Instant::now() + delay);
        let woken = hosting::next_wake(&self.hosting);
        hinted
            .into_iter()
            .chain(woken)
            .chain(carried)
            .chain(self.client.seats.repeat_due())
            .min()
    }

    fn settle_windows(&mut self) {
        let mut ids = std::mem::take(&mut self.settling);
        ids.extend(self.hosting.iter().map(|(id, _)| *id));
        for id in ids.drain(..) {
            self.settle_window(id);
        }
        self.settling = ids;
        if std::mem::take(&mut self.screens_moved) {
            self.publish_refresh();
        }
    }

    fn settle_window(&mut self, id: WindowId) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let arrived = hosted.state.arrived.take();
        let configures = arrived.is_some();
        let resized = arrived.is_some_and(|configured| {
            if let Some(server_side) = configured.server_side {
                hosted.state.decorate(&mut hosted.ui, !server_side);
            }
            hosted.state.configure(configured)
        });
        let rescaled = hosted.state.rescale(&self.client.outputs);
        if configures {
            self.touch(id);
        }
        if resized || rescaled {
            self.resize(id, rescaled);
        }
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        if std::mem::take(&mut hosted.state.close_requested) {
            match hosted.state.closing.ask(&mut hosted.ui) {
                true => self.close(id),
                false => self.touch(id),
            }
        }
    }

    fn resize(&mut self, id: WindowId, rescaled: bool) {
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let Some(drawn) = hosted.state.drawn() else {
            return;
        };
        let PixelSize { width, height } = drawn.scale.physical(drawn.logical.into());
        let scale = drawn.scale.factor();
        let interactive = hosted.state.is_resizing();
        hosted.ui.set_viewport(
            Size::new(drawn.logical.0 as f32, drawn.logical.1 as f32),
            scale as f32,
        );
        if hosted.has_surface() {
            hosted.adjust(move |presenter| presenter.resize((width, height), scale, interactive));
            hosted.ui.request_redraw();
        } else if let Err(error) = self.attach_presenter(id, (width, height), scale) {
            self.fail(error);
            return;
        }
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        let drawable = hosted.state.is_drawable();
        if drawable && (!hosted.is_drawable() || rescaled) {
            hosted.frame_answered();
            hosted.ui.request_redraw();
        }
        hosted.drawable(drawable);
    }

    fn attach_presenter(&mut self, id: WindowId, size: (u32, u32), scale: f64) -> Result<()> {
        let Some(hosted) = self.hosting.get(&id) else {
            return Ok(());
        };
        let surface = hosted.state.toplevel.surface().clone();
        let interactive = hosted.state.is_resizing();
        let config = SurfaceConfig {
            transparent: hosted.state.transparent(),
            ..SurfaceConfig::default()
        };
        let mut presenter = {
            let mut renderer = self.hosting.renderer().expect("renderer not initialized");
            Presenter::new(&self.client.host, &surface, &mut renderer, config)
                .map_err(rikka_renderer::Error::from)?
        };
        presenter.resize(size, scale, interactive);
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return Ok(());
        };
        hosted.attach(presenter);
        Ok(())
    }

    fn follow_seats(&mut self) {
        let Self {
            hosting, client, ..
        } = self;
        let mut locked = false;
        for (_, hosted) in hosting.iter() {
            let surface = hosted.state.toplevel.surface();
            client.seats.show_cursor(
                surface,
                hosted.ui.cursor(),
                hosted.state.scale().cursor_scale(),
            );
            locked |= hosted.state.is_locked();
        }
        client.seats.update_text_input_rects(|surface| {
            let id = hosting::window_of(hosting, surface)?;
            hosting.get(&id)?.ui.caret()
        });
        if locked != self.travelling {
            self.travelling = locked;
            self.client.seats.set_relative_motion(locked);
        }
    }

    pub(crate) fn publish_refresh(&self) {
        self.client.outputs.publish(
            self.hosting
                .iter()
                .flat_map(|(_, hosted)| hosted.state.outputs.iter()),
        );
    }

    fn heard_input(
        &mut self,
        surface: &rikka_wayland_client::wl_surface::WlSurface,
        event: Event,
        stylus: Option<Stylus>,
    ) {
        if let Event::KeyPressed { key, .. } | Event::KeyReleased { key, .. } = &event
            && *key == Key::Escape
            && self.dragging.is_active()
        {
            if matches!(event, Event::KeyPressed { .. }) {
                self.dragging.cancel(&self.session);
            }
            return;
        }
        let Some(id) = hosting::window_of(&self.hosting, surface) else {
            return;
        };
        let Some(hosted) = self.hosting.get_mut(&id) else {
            return;
        };
        if matches!(event, Event::PointerTravelled(_)) && !hosted.state.is_locked() {
            return;
        }
        hosted.ui.handle_input(event, stylus);
        self.touch(id);
    }
}

fn pad_input(event: PadEvent) -> Option<PadInput> {
    match event {
        PadEvent::Button {
            button, pressed, ..
        } => Some(PadInput::Button { button, pressed }),
        PadEvent::Ring {
            group,
            ring,
            degrees,
            ..
        } => Some(PadInput::Ring {
            group,
            ring,
            degrees,
        }),
        PadEvent::Strip {
            group,
            strip,
            position,
            ..
        } => Some(PadInput::Strip {
            group,
            strip,
            position,
        }),
        PadEvent::Dial {
            group,
            dial,
            value120,
            ..
        } => Some(PadInput::Dial {
            group,
            dial,
            value120,
        }),
        PadEvent::Entered | PadEvent::Left | PadEvent::Mode { .. } => None,
    }
}

impl SeatHost for Runner {
    fn seats(&mut self) -> &mut Seats {
        &mut self.client.seats
    }

    fn handle_event(&mut self, event: SeatEvent) {
        match event {
            SeatEvent::Input { surface, event } => self.heard_input(&surface, event.into(), None),
            SeatEvent::Stylus {
                surface,
                event,
                stylus,
            } => self.heard_input(&surface, event.into(), Some(stylus)),
            SeatEvent::Pad { surface, event } => {
                if let Some(input) = pad_input(event) {
                    self.heard_input(&surface, Event::Pad(input), None);
                }
            }
            SeatEvent::Keyboard { surface, entered } => {
                let Some(id) = hosting::window_of(&self.hosting, &surface) else {
                    return;
                };
                let Some(hosted) = self.hosting.get_mut(&id) else {
                    return;
                };
                if let Some(hook) = hosted.state.keyboard(entered) {
                    hosted.ui.start(hook);
                }
                self.touch(id);
            }
        }
    }
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("app_name", &self.app_name)
            .field("session", &self.session)
            .field("dragging", &self.dragging)
            .finish_non_exhaustive()
    }
}
