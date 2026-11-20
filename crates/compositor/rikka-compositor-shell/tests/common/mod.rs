pub(crate) mod client;

use std::ops::Deref;

use rikka_compositor_protocols::wl_data_device_manager::WlDataDeviceManager;
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_seat::WlSeat;
use rikka_compositor_server::{ClientOptions, DisplayHandle};
use rikka_compositor_shell::{
    ForeignToplevelList, LayerEvent, LayerId, LayerShell, LockEvent, LockId, LockSurfaceId,
    PopupEvent, PopupId, SessionLock, Shell, ShellEvent, ShellHost, TokenRequest, ToplevelEvent,
    ToplevelId, XdgShell, XwaylandShell,
};
use rikka_compositor_surface::{SurfaceHost, SurfaceId, SurfaceProtocols, Surfaces};
use rikka_test_display::{Canvas, Display, Host, Notes};
use rikka_wayland_client::wl_shm::Format;

pub(crate) use client::Wayland;

pub(crate) const ICON_SIZES: [i32; 2] = [32, 64];

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) surfaces: Surfaces,
    pub(crate) shell: Shell,
    pub(crate) toplevels: Vec<ToplevelId>,
    pub(crate) popups: Vec<PopupId>,
    pub(crate) layers: Vec<LayerId>,
    pub(crate) locks: Vec<LockId>,
    pub(crate) lock_surfaces: Vec<LockSurfaceId>,
    pub(crate) admit_activation: bool,
    pub(crate) activated: Vec<SurfaceId>,
    notes: Notes,
}

impl State {
    pub(crate) fn note(&self, line: impl Into<String>) {
        self.notes.note(line);
    }

    pub(crate) fn last_toplevel(&self) -> ToplevelId {
        *self.toplevels.last().expect("a toplevel was created")
    }

    pub(crate) fn last_popup(&self) -> PopupId {
        *self.popups.last().expect("a popup was created")
    }

    pub(crate) fn last_layer(&self) -> LayerId {
        *self.layers.last().expect("a layer surface was created")
    }
}

impl SurfaceHost for State {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }
}

impl ShellHost for State {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }

    fn shell_event(&mut self, event: ShellEvent) {
        match &event {
            ShellEvent::Toplevel {
                toplevel,
                event: ToplevelEvent::Created,
            } => self.toplevels.push(*toplevel),
            ShellEvent::Popup {
                popup,
                event: PopupEvent::Created,
            } => self.popups.push(*popup),
            ShellEvent::Layer {
                layer,
                event: LayerEvent::Created,
            } => self.layers.push(*layer),
            ShellEvent::Lock(LockEvent::Requested(lock)) => self.locks.push(*lock),
            ShellEvent::Lock(LockEvent::SurfaceCreated(surface)) => {
                self.lock_surfaces.push(*surface);
            }
            ShellEvent::Activate { surface, .. } => self.activated.push(*surface),
            _ => {}
        }
        self.note(format!("{event:?}"));
    }

    fn admit_activation(&mut self, request: &TokenRequest) -> bool {
        self.note(format!(
            "token requested with serial {:?} and app id {:?}",
            request.serial.as_ref().map(|(serial, _)| *serial),
            request.app_id
        ));
        self.admit_activation
    }
}

impl Host for State {
    fn display_handle(&self) -> &DisplayHandle<Self> {
        &self.display
    }
}

pub(crate) struct Server(Display<State>);

impl Server {
    pub(crate) fn start() -> Self {
        let display = Display::start(move |reactor, notes| {
            let display = DisplayHandle::new(reactor);
            display.install(SurfaceProtocols);
            display.install(XdgShell::new().icon_sizes(ICON_SIZES));
            display.install(LayerShell);
            display.install(SessionLock);
            display.install(ForeignToplevelList);
            display.install(XwaylandShell);
            serve_extras(&display);
            let shell = Shell::new(&display);
            State {
                display,
                surfaces: Surfaces::new(),
                shell,
                toplevels: Vec::new(),
                popups: Vec::new(),
                layers: Vec::new(),
                locks: Vec::new(),
                lock_surfaces: Vec::new(),
                admit_activation: true,
                activated: Vec::new(),
                notes,
            }
        });
        Self(display)
    }

    pub(crate) fn connect(&self) -> Wayland {
        self.connect_with(ClientOptions::new())
    }

    pub(crate) fn connect_with(&self, options: ClientOptions) -> Wayland {
        Wayland::new(self.0.connect(options))
    }

    pub(crate) fn connect_xwayland(&self) -> Wayland {
        self.connect_with(ClientOptions::new().xwayland())
    }
}

impl Deref for Server {
    type Target = Display<State>;

    fn deref(&self) -> &Display<State> {
        &self.0
    }
}

fn serve_extras(display: &DisplayHandle<State>) {
    display.serve::<WlOutput>(|_, _, _| {});
    display.serve::<WlSeat>(|_, _, _| {});
    display.serve::<WlDataDeviceManager>(|_, _, _| {});
    display.serve::<WlDataSource>(|_, _, _| {});
    display.global::<WlOutput>().version(4).publish();
    display.global::<WlSeat>().publish();
    display.global::<WlDataDeviceManager>().version(3).publish();
}

pub(crate) struct Mapped {
    pub(crate) window: client::Toplevel,
    pub(crate) id: ToplevelId,
    _buffer: Canvas,
}

pub(crate) fn map_toplevel(
    server: &Server,
    client: &mut Wayland,
    wm_base: &rikka_wayland_client::xdg_wm_base::XdgWmBase,
    tag: u32,
    size: (i32, i32),
) -> Mapped {
    let window = client.toplevel(wm_base, tag);
    window.surface.commit();
    client.roundtrip();
    let id = server.call(|state| state.last_toplevel());
    server.call(move |state| {
        state.shell.configure_toplevel(
            id,
            rikka_compositor_shell::ToplevelConfigure {
                size: Some(rikka_compositor_geometry::Size::new(size.0, size.1)),
                ..Default::default()
            },
        )
    });
    client.roundtrip();
    window
        .xdg_surface
        .ack_configure(client.received.last_configure(tag));
    let buffer = client.canvas(size.0, size.1, Format::Argb8888);
    window.surface.attach(Some(&buffer.buffer), 0, 0);
    window.surface.commit();
    client.roundtrip();
    Mapped {
        window,
        id,
        _buffer: buffer,
    }
}
