use rikka_blur::SurfaceRegions;
use rikka_instance::LaunchRequest;
use rikka_types::{Color, Size};
use rikka_ui::{Command, Handler, Ui, WindowChrome, WindowRef, WindowRequest, request_window};
use rikka_wayland::{Configures, Scale};
use rikka_wayland_client::wl_output::WlOutput;

use crate::sender::WindowAlive;
use crate::tasks::WindowId;
use crate::wayland::client::Client;
use crate::wayland::configure::{Configured, Configuring, States};
use crate::wayland::feedback::FeedbackTracker;
use crate::wayland::inhibit::ShortcutInhibitor;
use crate::wayland::locked::Locked;
use crate::wayland::outputs::Outputs;
use crate::wayland::scale::SurfaceSize;
use crate::wayland::session::Session;
use crate::wayland::toplevel::Toplevel;
use crate::window::{
    Closing, RequestedGeometry, Shaping, WindowHandlers, WindowShape, WindowTitle, set_chrome,
    set_client_side,
};

pub(crate) struct WindowState {
    pub(crate) id: WindowId,
    pub(crate) alive: WindowAlive,
    pub(crate) toplevel: Toplevel,
    pub(crate) chrome: WindowChrome,
    pub(crate) glazing: SurfaceRegions,
    pub(crate) needs_alpha: bool,
    transparent: bool,
    client_side: bool,
    pub(crate) focus: Option<Handler>,
    pub(crate) blur: Option<Handler>,
    language: Option<Handler>,
    pub(crate) closing: Closing,
    pub(crate) incoming: Configuring,
    pub(crate) arrived: Option<Configured>,
    pub(crate) close_requested: bool,
    pub(crate) outputs: Vec<WlOutput>,
    pub(crate) feedback: Option<FeedbackTracker>,
    requested: RequestedGeometry,
    window_title: WindowTitle,
    shape: Option<Handler<(WindowShape,)>>,
    open_handler: Option<Handler<(LaunchRequest,)>>,
    shaping: Shaping,
    raising: Vec<WindowRef>,
    quitting: bool,
    states: States,
    size: Option<(u32, u32)>,
    scale: Scale,
    configures: Configures<(u32, u32)>,
    sealed: Option<SurfaceSize>,
    committed: Option<SurfaceSize>,
    keyboards: u32,
    locked: Option<Locked>,
    inhibitor: Option<ShortcutInhibitor>,
    withheld: bool,
}

pub(crate) struct Parts {
    pub(crate) id: WindowId,
    pub(crate) toplevel: Toplevel,
    pub(crate) alive: WindowAlive,
    pub(crate) chrome: WindowChrome,
    pub(crate) glazing: SurfaceRegions,
    pub(crate) needs_alpha: bool,
    pub(crate) transparent: bool,
    pub(crate) client_side: bool,
    pub(crate) hooks: WindowHandlers,
    pub(crate) requested: RequestedGeometry,
    pub(crate) feedback: Option<FeedbackTracker>,
}

impl WindowState {
    pub(crate) fn new(parts: Parts) -> Self {
        let Parts {
            id,
            toplevel,
            alive,
            chrome,
            glazing,
            needs_alpha,
            transparent,
            client_side,
            hooks,
            requested,
            feedback,
        } = parts;
        Self {
            id,
            alive,
            toplevel,
            chrome,
            glazing,
            needs_alpha,
            transparent,
            focus: hooks.focus,
            blur: hooks.blur,
            language: hooks.language,
            closing: hooks.closing,
            incoming: Configuring::default(),
            arrived: None,
            close_requested: false,
            outputs: Vec::new(),
            feedback,
            client_side,
            requested,
            window_title: hooks.window_title,
            shape: hooks.shape,
            open_handler: hooks.open_handler,
            shaping: Shaping::new(requested.size),
            raising: Vec::new(),
            quitting: false,
            states: States::default(),
            size: None,
            scale: Scale::default(),
            configures: Configures::new(),
            sealed: None,
            committed: None,
            keyboards: 0,
            locked: None,
            inhibitor: None,
            withheld: false,
        }
    }

    pub(crate) fn is_drawable(&self) -> bool {
        !self.withheld && self.size.is_some()
    }

    pub(crate) fn withhold(&mut self, withheld: bool) {
        self.withheld = withheld;
    }

    pub(crate) fn scale(&self) -> Scale {
        self.scale
    }

    pub(crate) fn drawn(&self) -> Option<SurfaceSize> {
        Some(SurfaceSize {
            logical: self.size?,
            scale: self.scale,
        })
    }

    pub(crate) fn logical(&self) -> Size {
        let (width, height) = self.size.unwrap_or((
            whole(self.requested.size.width),
            whole(self.requested.size.height),
        ));
        Size::new(width as f32, height as f32)
    }

    pub(crate) fn configure(&mut self, configured: Configured) -> bool {
        let was_resizing = self.states.resizing;
        self.states = configured.states;
        let floating = !self.states.maximized && !self.states.fullscreen;
        let chosen = match configured.size {
            Some(size) if self.requested.resizable || !floating => size,
            _ => self
                .size
                .unwrap_or_else(|| self.first_size(configured.bounds)),
        };
        let resolved = match (self.requested.min_size, floating) {
            (Some(min), true) => (
                chosen.0.max(whole(min.width)),
                chosen.1.max(whole(min.height)),
            ),
            _ => chosen,
        };
        self.configures.push(configured.serial, resolved);
        let changed = self.size != Some(resolved);
        self.size = Some(resolved);
        changed || was_resizing != self.states.resizing
    }

    pub(crate) fn is_resizing(&self) -> bool {
        self.states.resizing
    }

    fn first_size(&self, bounds: Option<(u32, u32)>) -> (u32, u32) {
        let requested = (
            whole(self.requested.size.width).max(1),
            whole(self.requested.size.height).max(1),
        );
        match bounds {
            Some((width, height)) => (requested.0.min(width), requested.1.min(height)),
            None => requested,
        }
    }

    pub(crate) fn rescale(&mut self, outputs: &Outputs) -> bool {
        let scale = self
            .toplevel
            .scaling()
            .scale(|| outputs.scale_of(&self.outputs));
        let was = std::mem::replace(&mut self.scale, scale);
        was.factor() != scale.factor()
    }

    pub(crate) fn seal(&mut self) {
        self.sealed = self.drawn();
    }

    pub(crate) fn present(&mut self) {
        let Some(drawn) = self.sealed.take() else {
            return;
        };
        if let Some(serial) = self.configures.ack_for_commit(&drawn.logical) {
            self.toplevel.ack(serial);
        }
        self.toplevel.place(drawn);
        self.toplevel.frame(self.id);
        self.glazing.apply();
        if let Some(feedback) = &self.feedback {
            feedback.watch_next_commit();
        }
        self.committed = Some(drawn);
    }

    pub(crate) fn settle_unchanged(&mut self) {
        let Some(committed) = self.committed else {
            return;
        };
        if let Some(serial) = self.configures.ack_for_commit(&committed.logical) {
            self.toplevel.ack(serial);
            self.toplevel.commit();
        }
    }

    pub(crate) fn opened(&self, open: LaunchRequest) -> Option<Handler> {
        let handler = self.open_handler.clone()?;
        Some(handler.map(move |()| (open.clone(),)))
    }

    pub(crate) fn keyboard(&mut self, entered: bool) -> Option<Handler> {
        let was = self.keyboards > 0;
        self.keyboards = match entered {
            true => self.keyboards + 1,
            false => self.keyboards.saturating_sub(1),
        };
        match (was, self.keyboards > 0) {
            (false, true) => self.focus.clone(),
            (true, false) => self.blur.clone(),
            _ => None,
        }
    }

    pub(crate) fn decorate(&mut self, ui: &mut Ui, client_side: bool) {
        if self.client_side == client_side {
            return;
        }
        self.client_side = client_side;
        set_client_side(ui, client_side);
        self.sync_background(ui);
    }

    pub(crate) fn transparent(&self) -> bool {
        self.transparent
    }

    pub(crate) fn set_transparent(&mut self, ui: &mut Ui, transparent: bool) {
        self.transparent = transparent;
        self.sync_background(ui);
    }

    pub(crate) fn sync_background(&self, ui: &mut Ui) {
        ui.set_background((self.client_side && self.transparent).then_some(Color::TRANSPARENT));
    }

    pub(crate) fn is_locked(&self) -> bool {
        self.locked.is_some()
    }

    pub(crate) fn service(&mut self, ui: &mut Ui, session: &Session, client: &Client) -> bool {
        self.sync_title(ui);
        self.sync_shape(ui);
        self.sync_chrome(ui);
        self.sync_lock(ui, client);
        self.sync_grab(ui, session);
        self.serve_unclaimed(ui);
        self.apply_window_requests(ui, session, client)
    }

    fn serve_unclaimed(&mut self, ui: &mut Ui) {
        match ui.take_unclaimed_command() {
            Some(Command::Close) => ui.start(|| request_window(WindowRequest::AskToClose)),
            Some(Command::Quit) => self.quitting = true,
            Some(Command::Fullscreen) => {
                ui.start(|| request_window(WindowRequest::ToggleFullscreen));
            }
            _ => {}
        }
    }

    pub(crate) fn take_quitting(&mut self) -> bool {
        std::mem::take(&mut self.quitting)
    }

    pub(crate) fn sync_chrome(&mut self, ui: &mut Ui) {
        let current = (
            self.keyboards > 0,
            self.states.maximized,
            self.states.fullscreen,
            self.requested.resizable,
        );
        let shown = (
            self.chrome.focused,
            self.chrome.maximized,
            self.chrome.fullscreen,
            self.chrome.resizable,
        );
        if current == shown {
            return;
        }
        (
            self.chrome.focused,
            self.chrome.maximized,
            self.chrome.fullscreen,
            self.chrome.resizable,
        ) = current;
        set_chrome(ui, self.chrome.clone());
    }

    fn sync_shape(&mut self, ui: &mut Ui) {
        if self.size.is_none() {
            return;
        }
        let Some(shape) = self.shaping.update(
            self.logical(),
            self.states.maximized,
            self.states.fullscreen,
        ) else {
            return;
        };
        if let Some(hook) = &self.shape {
            ui.start(hook.clone().map(move |()| (shape,)));
        }
    }

    fn sync_title(&mut self, ui: &mut Ui) {
        let Some(title) = self.window_title.take() else {
            return;
        };
        if title == self.chrome.title {
            return;
        }
        self.toplevel.set_title(title.as_str());
        self.chrome.title = title;
        set_chrome(ui, self.chrome.clone());
    }

    pub(crate) fn relabel(&mut self, ui: &mut Ui) -> Option<Handler> {
        let title = self.chrome.title.retranslated();
        if title != self.chrome.title {
            self.toplevel.set_title(title.as_str());
            self.chrome.title = title;
            set_chrome(ui, self.chrome.clone());
        }
        self.language.clone()
    }

    fn sync_lock(&mut self, ui: &Ui, client: &Client) {
        let lock_requested = ui.pointer_locked();
        if lock_requested == self.locked.is_some() {
            return;
        }
        if !lock_requested {
            self.locked = None;
            return;
        }
        let surface = self.toplevel.surface();
        match (
            client.globals.constraints.as_ref(),
            client.seats.pointer_over(surface),
        ) {
            (Some(constraints), Some(pointer)) => {
                self.locked = Some(Locked::hold(constraints, surface, pointer, self.id));
            }
            _ => tracing::warn!("failed to lock pointer"),
        }
    }

    fn sync_grab(&mut self, ui: &Ui, session: &Session) {
        let grab_requested = ui.grabs_keys();
        if grab_requested == self.inhibitor.is_some() {
            return;
        }
        self.inhibitor = grab_requested
            .then(|| session.inhibit_shortcuts(self.toplevel.surface()))
            .flatten();
    }

    pub(crate) fn take_raising(&mut self) -> Vec<WindowRef> {
        std::mem::take(&mut self.raising)
    }

    fn apply_window_requests(&mut self, ui: &mut Ui, session: &Session, client: &Client) -> bool {
        let toplevel = &self.toplevel;
        let raising = &mut self.raising;
        let states = self.states;
        let grab = client.seats.grab();
        let grabbed = |act: &dyn Fn(&rikka_seat::Grab)| match grab {
            Some(grab) => act(grab),
            None => tracing::debug!("no button press serial for move or resize"),
        };
        self.closing.serve(ui, |request| match request {
            WindowRequest::Move => grabbed(&|grab| toplevel.start_move(grab)),
            WindowRequest::Resize(edge) => grabbed(&|grab| toplevel.start_resize(grab, edge)),
            WindowRequest::Minimize => toplevel.minimize(),
            WindowRequest::ToggleMaximize => toplevel.set_maximized(!states.maximized),
            WindowRequest::ShowMenu(position) => {
                grabbed(&|grab| toplevel.show_menu(grab, position));
            }
            WindowRequest::Fullscreen(on) => toplevel.set_fullscreen(on),
            WindowRequest::ToggleFullscreen => toplevel.set_fullscreen(!states.fullscreen),
            WindowRequest::Activate(token) => session.activate(toplevel.surface(), &token),
            WindowRequest::Raise(target) => raising.push(target),
            WindowRequest::AskToClose | WindowRequest::Close => {}
        })
    }
}

fn whole(logical: f32) -> u32 {
    logical.round().max(0.0) as u32
}
