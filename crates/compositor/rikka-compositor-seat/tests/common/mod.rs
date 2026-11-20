pub(crate) mod client;

use std::ops::Deref;
use std::time::Duration;

use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_seat::{
    KeyFilter, KeyboardFocus, Keymap, Keystroke, SeatEvent, SeatHost, SeatId, SeatMut,
    SeatProtocols, Seats,
};
use rikka_compositor_server::{ClientOptions, DisplayHandle};
use rikka_compositor_surface::{Committed, SurfaceHost, SurfaceId, SurfaceProtocols, Surfaces};
use rikka_compositor_types::Press;
pub(crate) use rikka_test_display::PATIENCE;
use rikka_test_display::{Display, Host, Notes};

pub(crate) use client::{Received, Wayland};

pub(crate) const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "rikka" {
        minimum = 8;
        maximum = 255;
        <ESC> = 9;
        <AD01> = 24;
        <AC01> = 38;
        <LFSH> = 50;
        <LWIN> = 133;
    };
    xkb_types "rikka" {
        type "ONE_LEVEL" {
            modifiers = none;
            level_name[Level1] = "Any";
        };
        type "TWO_LEVEL" {
            modifiers = Shift;
            map[Shift] = Level2;
            level_name[Level1] = "Base";
            level_name[Level2] = "Shift";
        };
    };
    xkb_compatibility "rikka" {
        interpret Shift_L { action = SetMods(modifiers = Shift); };
        interpret Super_L { action = SetMods(modifiers = Mod4); };
    };
    xkb_symbols "rikka" {
        key <ESC> { [ Escape ] };
        key <AD01> { type = "TWO_LEVEL", [ q, Q ] };
        key <AC01> { type = "TWO_LEVEL", [ a, A ] };
        key <LFSH> { [ Shift_L ] };
        key <LWIN> { [ Super_L ] };
        modifier_map Shift { <LFSH> };
        modifier_map Mod4 { <LWIN> };
    };
};
"#;

pub(crate) const A: u32 = 30;
pub(crate) const Q: u32 = 16;
pub(crate) const SHIFT: u32 = 42;

pub(crate) fn keymap() -> Keymap {
    Keymap::from_text(KEYMAP).unwrap()
}

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) surfaces: Surfaces,
    pub(crate) seats: Seats,
    pub(crate) seat_mut: SeatId,
    pub(crate) bindings: Vec<u32>,
    pub(crate) strokes: Vec<Keystroke>,
    pub(crate) events: Vec<SeatEvent>,
    notes: Notes,
}

impl State {
    pub(crate) fn note(&self, line: impl Into<String>) {
        self.notes.note(line);
    }

    pub(crate) fn surface(&self, protocol_id: u32) -> SurfaceId {
        let found: Vec<SurfaceId> = self
            .display
            .clients()
            .iter()
            .filter_map(|client| client.object::<WlSurface>(protocol_id))
            .filter_map(|surface| self.surfaces.lookup(&surface))
            .collect();
        assert_eq!(found.len(), 1, "surface #{protocol_id} names one surface");
        found[0]
    }

    pub(crate) fn seat_mut(&mut self) -> SeatMut<'_, State> {
        let seat_mut = self.seat_mut;
        rikka_compositor_seat::seat(self, seat_mut)
    }

    pub(crate) fn focus_keyboard(&mut self, protocol_id: u32) -> SurfaceId {
        let surface = self.surface(protocol_id);
        self.seat_mut()
            .set_keyboard_focus(Some(KeyboardFocus::Surface(surface)));
        surface
    }

    pub(crate) fn type_key(&mut self, code: u32, press: Press) {
        self.seat_mut().key(rikka_compositor_seat::Key {
            code,
            press,
            time: Duration::from_millis(7),
            source: rikka_compositor_seat::KeySource::Device(rikka_compositor_seat::InputDevice(1)),
        });
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

    fn filter_key(&mut self, keystroke: &Keystroke) -> KeyFilter {
        self.strokes.push(keystroke.clone());
        self.note(format!(
            "keystroke {} {:?}",
            keystroke.code, keystroke.press
        ));
        if self.bindings.contains(&keystroke.code) && !keystroke.shortcuts_inhibited {
            KeyFilter::Consume
        } else {
            KeyFilter::Forward
        }
    }

    fn seat_event(&mut self, event: SeatEvent) {
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
            let mut seats = Seats::new();
            let seat_mut = seats.add(&display, "seat-0", keymap());
            State {
                display,
                surfaces: Surfaces::new(),
                seats,
                seat_mut,
                bindings: Vec::new(),
                strokes: Vec::new(),
                events: Vec::new(),
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
        Wayland::new(self.0.connect(options))
    }
}

impl Deref for Server {
    type Target = Display<State>;

    fn deref(&self) -> &Display<State> {
        &self.0
    }
}
