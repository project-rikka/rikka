use rikka_compositor_output::{
    BackgroundEffectProtocols, BackgroundEffects, CaptureProtocols, Captures, DrmLeaseProtocols,
    OutputProtocols,
};
use rikka_compositor_pixels::SHM_FORMATS;
use rikka_compositor_reactor::Reactor;
use rikka_compositor_seat::{SeatProtocols, Seats};
use rikka_compositor_selection::SelectionProtocols;
use rikka_compositor_server::{Client, DisplayHandle};
use rikka_compositor_shell::{
    ForeignToplevelList, LayerShell, SessionLock, Shell, XdgShell, XwaylandShell,
};
use rikka_compositor_surface::{SurfaceProtocols, Surfaces};
use rikka_session::agent;
use rikka_session::profile::Profile;

use crate::animation::Animations;
use crate::backend::{Backend, Extent, Headless, Kms, Nested};
use crate::capture::Capture;
use crate::context::lock_ctx;
use crate::context::{session_ctx, session_ref, switcher_ctx};
use crate::cursor::Cursor;
use crate::error::{Error, Result};
use crate::input::{Bindings, Input, PERSON_SEAT, PointerRules, compile_keymap};
use crate::logging::Journal;
use crate::outputs::{FALLBACK_REFRESH, Outputs};
use crate::policy::Policy;
use crate::render::{FrameScheduler, Resting, Scenes, Throttle, TimelineWatch};
use crate::selection::Selection;
use crate::session::{
    self, Desktop, FIRST_LOCKER_TIMEOUT, NiceCapability, PrivateBus, Session, SessionKind,
    raise_frame_loop,
};
use crate::settings::SessionSettings;
use crate::shell::Locking;
use crate::snap::Snapping;
use crate::state::{Compositor, Core};
use crate::switcher::Switcher;
use crate::wayland::{
    Leasing, Presence, SandboxProtocol, Sandboxes, Views, WindowViewProtocol, WorkspaceList,
    WorkspaceProtocol, enable_gpu_protocols,
};
use crate::windows::Windows;
use crate::x11::X11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screens {
    Kms,
    Nested,
    Headless {
        extent: Extent,
        millihertz: u32,
        bus: Bus,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bus {
    Host,
    Private,
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Launch {
    screens: Screens,
    profile: Profile,
    desktop: bool,
    locked: bool,
    socket: Option<String>,
}

impl Launch {
    pub fn kms() -> Self {
        Self::new(Screens::Kms)
    }

    pub fn nested() -> Self {
        Self::new(Screens::Nested)
    }

    pub fn headless(extent: Extent) -> Self {
        Self::headless_at(extent, FALLBACK_REFRESH)
    }

    pub fn headless_at(extent: Extent, millihertz: u32) -> Self {
        Self::new(Screens::Headless {
            extent,
            millihertz,
            bus: Bus::Host,
        })
    }

    fn new(screens: Screens) -> Self {
        let profile = match screens {
            Screens::Kms | Screens::Nested => Profile::Full,
            Screens::Headless { .. } => Profile::Simple,
        };
        Self {
            screens,
            profile,
            desktop: true,
            locked: false,
            socket: None,
        }
    }

    pub fn profile(mut self, profile: Profile) -> Self {
        self.profile = profile;
        self
    }

    pub fn desktop(mut self, starts: bool) -> Self {
        self.desktop = starts;
        self
    }

    pub fn locked(mut self, starts_locked: bool) -> Self {
        self.locked = starts_locked;
        self
    }

    pub fn socket(mut self, name: impl Into<String>) -> Self {
        self.socket = Some(name.into());
        self
    }

    pub fn private_bus(mut self) -> Self {
        if let Screens::Headless { bus, .. } = &mut self.screens {
            *bus = Bus::Private;
        }
        self
    }

    pub fn journal(&self) -> Journal {
        match self.screens {
            Screens::Kms => Journal::Session,
            Screens::Nested => Journal::Nested,
            Screens::Headless { .. } => Journal::Headless,
        }
    }

    pub fn start(self) -> Result<Compositor> {
        let nice = NiceCapability::without_inheritance();
        if !matches!(self.screens, Screens::Headless { .. }) {
            nice.foretell();
        }
        let profile = self.profile;
        if self.screens == Screens::Kms {
            raise_frame_loop();
        }

        let reactor = Reactor::new().map_err(Error::EventLoop)?;
        let display = DisplayHandle::new(&reactor);
        let mut surfaces = Surfaces::new();
        surfaces.set_shm_formats(SHM_FORMATS);
        let socket = display
            .listen(self.socket.as_deref())
            .map_err(Error::Socket)?;
        tracing::info!("listening on WAYLAND_DISPLAY={}", socket.name());

        let standing = standing(self.screens, socket.name())?;
        let desktop = Desktop::new(self.journal(), socket.name());
        let mut session =
            Session::new(&reactor, profile, standing, desktop).map_err(Error::EventLoop)?;
        install_protocols(&display, profile);
        let mut settings = SessionSettings::start(session.seam.seam(), profile, &session.standing);
        let policy = settings
            .as_mut()
            .map_or_else(|| Policy::declared(profile), Policy::read);
        let rules = settings
            .as_ref()
            .map_or_else(PointerRules::declared, SessionSettings::pointer_rules);
        let sealed = self.locked
            && settings
                .as_ref()
                .is_none_or(SessionSettings::locks_at_login);
        let keymap = compile_keymap(policy.keymap())?;
        let mut cursor = Cursor::new();
        cursor.set_theme(rules.cursor.theme.as_deref(), rules.cursor.size);

        let mut outputs = Outputs::default();
        let mut backend: Box<dyn Backend> = match self.screens {
            Screens::Kms => Box::new(Kms::start(&reactor, &mut outputs, rules.devices(), nice)?),
            Screens::Nested => {
                Box::new(Nested::start(&reactor, &mut outputs, nice, socket.name())?)
            }
            Screens::Headless {
                extent, millihertz, ..
            } => {
                nice.leave_state_thread();
                Box::new(Headless::start(&reactor, &mut outputs, extent, millihertz)?)
            }
        };
        let bindings = Bindings::read(settings.as_ref(), backend.switches_vt());
        let mut seats = Seats::new();
        let person = seats.add(&display, PERSON_SEAT, keymap.clone());
        let helper = seats.add_carried(&display, agent::SEAT, keymap, person, Client::is_agent);
        let input = Input::new(rules, bindings, person, helper);

        let syncobj = backend.syncobj_device();
        enable_gpu_protocols(&display, &mut surfaces, backend.renderer(), syncobj);
        let watch = TimelineWatch::start(&reactor, backend.renderer().timeline())
            .map_err(Error::EventLoop)?;
        let effects = BackgroundEffects::default();
        let shell = Shell::new(&display);
        let windows = Windows::new(profile, policy.window_rules(), outputs.wayland().layouts());

        let mut compositor = Compositor {
            core: Core {
                reactor: reactor.clone(),
                display,
                socket,
                profile,
                running: true,
            },
            backend,
            outputs,
            surfaces,
            shell,
            seats,
            windows,
            locking: Locking::default(),
            presence: Presence::default(),
            frames: FrameScheduler::new(&reactor),
            scenes: Scenes::new(watch, policy.decor()),
            throttle: Throttle::new(&reactor),
            resting: Resting::new(&reactor),
            captures: Captures::new(),
            capture: Capture::default(),
            effects,
            leasing: Leasing::default(),
            sandboxes: Sandboxes::default(),
            workspace_list: WorkspaceList::default(),
            views: Views::default(),
            cursor,
            input,
            selection: Selection::default(),
            animations: Animations::following(policy.animates()),
            snap: Snapping::default(),
            switcher: Switcher::default(),
            policy,
            settings,
            session,
            x11: X11::default(),
        };
        switcher_ctx!(compositor).seat_up();
        switcher_ctx!(compositor).offer_input_seams();
        reactor.each_turn(Compositor::each_turn);
        reactor.after_each(Compositor::settle);
        compositor.displays_detected();
        let name = compositor.socket_name().to_owned();
        compositor.session.seam.listen(&name);
        compositor
            .input
            .agent
            .listen(&compositor.core.display, &name);
        if sealed {
            lock_ctx!(compositor).set_session_locked();
            session_ctx!(compositor).await_locker(FIRST_LOCKER_TIMEOUT);
        }
        session_ctx!(compositor).start_xwayland();
        session_ref!(compositor).publish_session_environment();
        if self.desktop {
            session_ctx!(compositor).start_desktop();
        }
        Ok(compositor)
    }
}

fn standing(screens: Screens, socket: &str) -> Result<SessionKind> {
    let start_bus = || {
        let directory = session::directory(socket).ok_or_else(|| {
            Error::Bus(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "XDG_RUNTIME_DIR is not set",
            ))
        })?;
        PrivateBus::start(&directory).map_err(Error::Bus)
    };
    match screens {
        Screens::Kms => Ok(SessionKind::Login),
        Screens::Headless { bus: Bus::Host, .. } => Ok(SessionKind::Headless(None)),
        Screens::Headless {
            bus: Bus::Private, ..
        } => start_bus().map(|bus| SessionKind::Headless(Some(bus))),
        Screens::Nested => start_bus().map(SessionKind::Nested),
    }
}

fn install_protocols(display: &DisplayHandle<Compositor>, profile: Profile) {
    display.install(SurfaceProtocols);
    display.install(XdgShell::new());
    display.install(LayerShell);
    display.install(SessionLock);
    display.install(ForeignToplevelList);
    display.install(XwaylandShell);
    display.install(SeatProtocols);
    display.install(OutputProtocols);
    display.install(CaptureProtocols);
    display.install(BackgroundEffectProtocols);
    display.install(DrmLeaseProtocols);
    display.install(SelectionProtocols);
    display.install(SandboxProtocol);
    if profile.is_full() {
        display.install(WorkspaceProtocol);
        display.install(WindowViewProtocol);
    }
}
