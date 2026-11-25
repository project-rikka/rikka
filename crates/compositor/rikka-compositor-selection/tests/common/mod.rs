use std::io::Write;
pub(crate) mod client;

use std::ops::Deref;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use rikka_compositor_geometry::Point;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_seat::{
    Button, Focus, KeyboardFocus, Keymap, Motion, SeatEvent, SeatHost, SeatId, SeatMut,
    SeatProtocols, Seats,
};
use rikka_compositor_selection::{
    MimeList, SelectionEvent, SelectionHost, SelectionKind, SelectionProtocols, Selections,
    SendDrag, SendSelection,
};
use rikka_compositor_server::{Client, ClientId, ClientOptions, DisplayHandle};
use rikka_compositor_shell::{
    Shell, ShellEvent, ShellHost, TokenRequest, ToplevelEvent, ToplevelId, XdgShell,
};
use rikka_compositor_surface::{Committed, SurfaceHost, SurfaceId, SurfaceProtocols, Surfaces};
use rikka_compositor_types::Press;
use rikka_test_display::{Display, Host, Notes};

pub(crate) use client::{Wayland, drain, pipe};

pub(crate) fn mimes(types: &[&str]) -> MimeList {
    types
        .iter()
        .map(|mime_type| Box::from(*mime_type))
        .collect()
}

pub(crate) const LEFT: u32 = 0x110;

pub(crate) const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "rikka" {
        minimum = 8;
        maximum = 255;
        <ESC> = 9;
        <LFSH> = 50;
    };
    xkb_types "rikka" {
        type "ONE_LEVEL" {
            modifiers = none;
            level_name[Level1] = "Any";
        };
    };
    xkb_compatibility "rikka" {
        interpret Shift_L { action = SetMods(modifiers = Shift); };
    };
    xkb_symbols "rikka" {
        key <ESC> { [ Escape ] };
        key <LFSH> { [ Shift_L ] };
        modifier_map Shift { <LFSH> };
    };
};
"#;

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) surfaces: Surfaces,
    pub(crate) seats: Seats,
    pub(crate) shell: Shell,
    pub(crate) selections: Selections,
    pub(crate) seat_mut: SeatId,
    pub(crate) carried: SeatId,
    pub(crate) trusted: bool,
    pub(crate) events: Vec<SelectionEvent>,
    pub(crate) sent: Vec<String>,
    pub(crate) foreign: Vec<SurfaceId>,
    pub(crate) toplevels: Vec<ToplevelId>,
    notes: Notes,
}

impl State {
    pub(crate) fn note(&self, line: impl Into<String>) {
        self.notes.note(line);
    }

    pub(crate) fn surface(&self, client: ClientId, protocol_id: u32) -> SurfaceId {
        self.display
            .client(client)
            .and_then(|client| client.object::<WlSurface>(protocol_id))
            .and_then(|surface| self.surfaces.lookup(&surface))
            .expect("the surface exists on the server")
    }

    pub(crate) fn seat_mut(&mut self) -> SeatMut<'_, State> {
        let seat_mut = self.seat_mut;
        rikka_compositor_seat::seat(self, seat_mut)
    }

    pub(crate) fn focus_keyboard(&mut self, client: ClientId, protocol_id: u32) {
        let surface = self.surface(client, protocol_id);
        self.seat_mut()
            .set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
    }

    pub(crate) fn focus_carried(&mut self, client: ClientId, protocol_id: u32) {
        let surface = self.surface(client, protocol_id);
        let carried = self.carried;
        rikka_compositor_seat::seat(self, carried)
            .set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
    }

    pub(crate) fn move_to(&mut self, under: Option<(ClientId, u32, (f64, f64))>, at: (f64, f64)) {
        let under = under.map(|(client, id, origin)| {
            Focus::new(self.surface(client, id), Point::new(origin.0, origin.1))
        });
        self.seat_mut().motion(Motion {
            location: Point::new(at.0, at.1),
            under,
            time: Duration::from_millis(3),
        });
        self.seat_mut().frame();
    }

    pub(crate) fn press(&mut self, press: Press) -> u32 {
        let serial = self.seat_mut().button(Button {
            code: LEFT,
            press,
            time: Duration::from_millis(4),
        });
        self.seat_mut().frame();
        serial
    }

    pub(crate) fn has_event(&self, event: &SelectionEvent) -> bool {
        self.events.contains(event)
    }
}

impl SurfaceHost for State {
    fn surfaces(&mut self) -> &mut Surfaces {
        &mut self.surfaces
    }

    fn committed(&mut self, commit: &Committed) {
        rikka_compositor_seat::surface_committed(self, commit);
    }

    fn destroyed(&mut self, surface: SurfaceId) {
        rikka_compositor_seat::surface_destroyed(self, surface);
    }
}

impl SeatHost for State {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn display(&self) -> &DisplayHandle<Self> {
        &self.display
    }

    fn seat_event(&mut self, event: SeatEvent) {
        rikka_compositor_selection::seat_event(self, &event);
    }
}

impl ShellHost for State {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }

    fn shell_event(&mut self, event: ShellEvent) {
        if let ShellEvent::Toplevel {
            toplevel,
            event: ToplevelEvent::Created,
        } = event
        {
            self.toplevels.push(toplevel);
        }
    }

    fn admit_activation(&mut self, _request: &TokenRequest) -> bool {
        false
    }
}

impl SelectionHost for State {
    fn selections(&mut self) -> &mut Selections {
        &mut self.selections
    }

    fn trusts_selection(&mut self, _client: &Client, _seat: SeatId, _kind: SelectionKind) -> bool {
        self.trusted
    }

    fn send_selection(&mut self, request: SendSelection) {
        let mut file = std::fs::File::from(request.fd);
        write!(file, "compositor {}", request.mime_type).unwrap();
        self.sent.push(request.mime_type);
    }

    fn send_drag(&mut self, request: SendDrag) {
        let mut file = std::fs::File::from(request.fd);
        write!(file, "carried {}", request.mime_type).unwrap();
        self.sent.push(request.mime_type);
    }

    fn foreign_drag_target(&mut self, surface: SurfaceId) -> bool {
        self.foreign.contains(&surface)
    }

    fn selection_event(&mut self, event: SelectionEvent) {
        self.note(format!("{event:?}"));
        self.events.push(event);
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
            display.install(SeatProtocols);
            display.install(XdgShell::new());
            display.install(SelectionProtocols);
            let shell = Shell::new(&display);
            let mut seats = Seats::new();
            let seat_mut = seats.add(&display, "seat-0", Keymap::from_text(KEYMAP).unwrap());
            let carried = seats.add_carried(
                &display,
                "seat-agent",
                Keymap::from_text(KEYMAP).unwrap(),
                seat_mut,
                Client::is_agent,
            );
            State {
                display,
                surfaces: Surfaces::new(),
                seats,
                shell,
                selections: Selections::new(),
                seat_mut,
                carried,
                trusted: false,
                events: Vec::new(),
                sent: Vec::new(),
                foreign: Vec::new(),
                toplevels: Vec::new(),
                notes,
            }
        });
        Self(display)
    }

    pub(crate) fn connect(&self) -> Wayland {
        self.connect_with(ClientOptions::new())
    }

    pub(crate) fn connect_unprivileged(&self) -> Wayland {
        self.connect_with(ClientOptions::new().unprivileged())
    }

    fn connect_with(&self, options: ClientOptions) -> Wayland {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let client =
            self.call(move |state| state.display.insert_client(theirs, options).unwrap().id());
        Wayland::new(ours, client)
    }
}

impl Deref for Server {
    type Target = Display<State>;

    fn deref(&self) -> &Display<State> {
        &self.0
    }
}
