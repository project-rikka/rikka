use std::ops::Deref;

use rikka_compositor_protocols::wl_surface::WlSurface as ServerSurface;
use rikka_compositor_seat::{
    KeyFilter, Keymap, Keystroke, SeatEvent as ServerEvent, SeatHost as ServerHost, SeatId,
    SeatMut, SeatProtocols, Seats as ServerSeats,
};
use rikka_compositor_server::{ClientOptions, DisplayHandle};
use rikka_compositor_surface::{Committed, SurfaceHost, SurfaceId, SurfaceProtocols, Surfaces};
use rikka_seat::{SeatEvent, SeatHost, Seats};
use rikka_test_display::{Display as TestDisplay, Host as TestHost};
use rikka_wayland::{Display, EventLoop};
use rikka_wayland_client::from_socket;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_registry::{self, WlRegistry};
use rikka_wayland_client::wl_surface::WlSurface;

const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "rikka" {
        minimum = 8;
        maximum = 255;
        <ESC> = 9;
    };
    xkb_types "rikka" {
        type "ONE_LEVEL" {
            modifiers = none;
            level_name[Level1] = "Any";
        };
    };
    xkb_compatibility "rikka" {
    };
    xkb_symbols "rikka" {
        key <ESC> { [ Escape ] };
    };
};
"#;

pub(crate) struct Compositor {
    display: DisplayHandle<Compositor>,
    surfaces: Surfaces,
    seats: ServerSeats,
    seat: SeatId,
}

impl Compositor {
    pub(crate) fn seat(&mut self) -> SeatMut<'_, Compositor> {
        let seat = self.seat;
        rikka_compositor_seat::seat(self, seat)
    }

    pub(crate) fn surface(&self, protocol_id: u32) -> SurfaceId {
        self.display
            .clients()
            .iter()
            .filter_map(|client| client.object::<ServerSurface>(protocol_id))
            .find_map(|surface| self.surfaces.lookup(&surface))
            .expect("the client's surface is known")
    }
}

impl SurfaceHost for Compositor {
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

impl ServerHost for Compositor {
    fn seats(&mut self) -> &mut ServerSeats {
        &mut self.seats
    }

    fn display(&self) -> &DisplayHandle<Self> {
        &self.display
    }

    fn filter_key(&mut self, _: &Keystroke) -> KeyFilter {
        KeyFilter::Forward
    }

    fn seat_event(&mut self, _: ServerEvent) {}
}

impl TestHost for Compositor {
    fn display_handle(&self) -> &DisplayHandle<Self> {
        &self.display
    }
}

pub(crate) struct Server(TestDisplay<Compositor>);

impl Server {
    pub(crate) fn start() -> Server {
        Server(TestDisplay::start(|reactor, _| {
            let display = DisplayHandle::new(reactor);
            display.install(SurfaceProtocols);
            display.install(SeatProtocols);
            let mut seats = ServerSeats::new();
            let keymap = Keymap::from_text(KEYMAP).expect("the test keymap compiles");
            let seat = seats.add(&display, "seat-0", keymap);
            Compositor {
                display,
                surfaces: Surfaces::new(),
                seats,
                seat,
            }
        }))
    }
}

impl Deref for Server {
    type Target = TestDisplay<Compositor>;

    fn deref(&self) -> &TestDisplay<Compositor> {
        &self.0
    }
}

pub(crate) struct Host {
    seats: Seats,
    pub(crate) events: Vec<SeatEvent>,
}

impl SeatHost for Host {
    fn seats(&mut self) -> &mut Seats {
        &mut self.seats
    }

    fn handle_event(&mut self, event: SeatEvent) {
        self.events.push(event);
    }
}

pub(crate) struct Client {
    pub(crate) host: Host,
    events: EventLoop<Host>,
    pub(crate) surface: WlSurface,
}

impl Client {
    pub(crate) fn connect(server: &Server) -> Client {
        let stream = server.connect(ClientOptions::new());
        let (connection, wl_display) = from_socket(stream).expect("the socket speaks wayland");
        let display =
            Display::from_connection(connection, wl_display).expect("the display answers");
        let mut events = display.event_loop::<Host>();
        Seats::listen(events.queue_mut());
        events.queue_mut().on::<WlRegistry>(on_wl_registry);
        let route = events.route();
        let seats = Seats::builder().bind(&display, &route);
        display.announce_globals(&route);
        let compositor: WlCompositor = display.bind(1..=6).expect("wl_compositor is announced");
        let surface = compositor.create_surface();
        surface.commit();
        let mut client = Client {
            host: Host {
                seats,
                events: Vec::new(),
            },
            events,
            surface,
        };
        client.settle();
        client.settle();
        client
    }

    pub(crate) fn surface_id(&self) -> u32 {
        self.surface.id()
    }

    pub(crate) fn seats(&self) -> &Seats {
        &self.host.seats
    }

    pub(crate) fn settle(&mut self) {
        self.events
            .roundtrip(&mut self.host)
            .expect("the compositor answers");
        Seats::flush(&mut self.host);
    }

    pub(crate) fn take(&mut self) -> Vec<SeatEvent> {
        self.settle();
        std::mem::take(&mut self.host.events)
    }
}

fn on_wl_registry(host: &mut Host, registry: WlRegistry, event: wl_registry::Event) {
    if let wl_registry::Event::Global {
        name,
        interface,
        version,
    } = event
        && interface == "wl_seat"
    {
        host.seats.add(&registry, name, version);
    }
}
