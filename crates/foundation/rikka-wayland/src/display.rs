use rikka_sync::Mutex;
use std::ops::RangeInclusive;
use std::path::Path;
use std::sync::Arc;

use rikka_wayland_client::wl_callback::WlCallback;
use rikka_wayland_client::wl_display::WlDisplay;
use rikka_wayland_client::wl_registry::{self, WlRegistry};
use rikka_wayland_client::wl_seat::{self, WlSeat};
use rikka_wayland_client::{Connection, Global, Proxy, Route};

use crate::error::{Error, Result};
use crate::event_loop::EventLoop;

pub(crate) const SEAT_VERSION: u32 = 9;
pub(crate) const SEAT_RELEASE_SINCE_VERSION: u32 = 5;

type Globals = Arc<Mutex<Vec<Global>>>;

#[derive(Clone)]
pub struct Display {
    connection: Connection,
    display: WlDisplay,
    registry: WlRegistry,
    globals: Globals,
}

impl Display {
    pub fn from_env() -> Result<Display> {
        let (connection, display) = rikka_wayland_client::connect_to_env()?;
        Display::from_connection(connection, display)
    }

    pub fn connect_to(socket: &Path) -> Result<Display> {
        let (connection, display) = rikka_wayland_client::connect_to(socket)?;
        Display::from_connection(connection, display)
    }

    pub fn from_connection(connection: Connection, display: WlDisplay) -> Result<Display> {
        let globals = Globals::default();
        let announced = Arc::clone(&globals);
        let registry = display
            .via(&Route::tap::<WlRegistry>(move |_, event| {
                let mut globals = announced.lock();
                match event {
                    wl_registry::Event::Global {
                        name,
                        interface,
                        version,
                    } => globals.push(Global {
                        name,
                        interface: interface.into(),
                        version,
                    }),
                    wl_registry::Event::GlobalRemove { name } => {
                        globals.retain(|global| global.name != name);
                    }
                    _ => {}
                }
            }))
            .get_registry();
        let display = Display {
            connection,
            display,
            registry,
            globals,
        };
        display.roundtrip()?;
        Ok(display)
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn announce_globals(&self, route: &Route) {
        self.display.via(route).get_registry();
    }

    #[must_use]
    pub fn sync(&self, route: &Route) -> WlCallback {
        self.display.via(route).sync()
    }

    pub fn bind<P: Proxy>(&self, versions: RangeInclusive<u32>) -> Result<P> {
        self.bind_on(&Route::discard(), versions)
    }

    pub fn bind_on<P: Proxy>(&self, route: &Route, versions: RangeInclusive<u32>) -> Result<P> {
        let global = self
            .globals_named(P::INTERFACE.name)
            .into_iter()
            .find(|global| global.version >= *versions.start())
            .ok_or_else(Error::missing::<P>)?;
        Ok(self.bind_global(route, &global, *versions.end()))
    }

    #[must_use]
    pub fn bind_optional<P: Proxy>(&self, versions: RangeInclusive<u32>) -> Option<P> {
        self.bind_optional_on(&Route::discard(), versions)
    }

    #[must_use]
    pub fn bind_optional_on<P: Proxy>(
        &self,
        route: &Route,
        versions: RangeInclusive<u32>,
    ) -> Option<P> {
        self.bind_on(route, versions)
            .inspect_err(|error| {
                tracing::debug!(interface = P::INTERFACE.name, %error, "optional global unavailable");
            })
            .ok()
    }

    #[must_use]
    pub fn bind_all<P: Proxy>(&self, route: &Route, version: u32) -> Vec<P> {
        self.globals_named(P::INTERFACE.name)
            .iter()
            .map(|global| self.bind_global(route, global, version))
            .collect()
    }

    pub fn seat(&self, name: Option<&str>) -> Result<WlSeat> {
        let names = Arc::new(Mutex::new(Vec::new()));
        let named = Arc::clone(&names);
        let route = Route::tap::<WlSeat>(move |seat, event| {
            if let wl_seat::Event::Name { name } = event {
                named.lock().push((seat.id(), name));
            }
        });
        let bound: Vec<WlSeat> = self.bind_all(&route, SEAT_VERSION);
        let chosen = match name {
            None => keep_one(bound, |index, _| index == 0).ok_or(Error::NoSeat)?,
            Some(name) => {
                self.roundtrip()?;
                let wanted = names
                    .lock()
                    .iter()
                    .find(|(_, seat_name)| seat_name == name)
                    .map(|(seat, _)| *seat);
                keep_one(bound, |_, seat| Some(seat.id()) == wanted)
                    .ok_or_else(|| Error::SeatNotFound(name.to_owned()))?
            }
        };
        Ok(chosen.via(&Route::discard()))
    }

    #[must_use]
    pub fn event_loop<S>(&self) -> EventLoop<S> {
        EventLoop::new(&self.connection)
    }

    pub fn flush(&self) -> Result<()> {
        self.connection.flush().map_err(Error::from)
    }

    pub(crate) fn roundtrip(&self) -> Result<()> {
        self.connection
            .queue::<()>()
            .roundtrip(&mut ())
            .map_err(Error::from)
    }

    pub(crate) fn offers(&self, interface: &str) -> bool {
        !self.globals_named(interface).is_empty()
    }

    fn bind_global<P: Proxy>(&self, route: &Route, global: &Global, version: u32) -> P {
        self.registry
            .via(route)
            .bind(global.name, version.min(global.version))
    }

    fn globals_named(&self, interface: &str) -> Vec<Global> {
        self.globals
            .lock()
            .iter()
            .filter(|global| &*global.interface == interface)
            .cloned()
            .collect()
    }
}

fn keep_one(seats: Vec<WlSeat>, keep: impl Fn(usize, &WlSeat) -> bool) -> Option<WlSeat> {
    let mut chosen = None;
    for (index, seat) in seats.into_iter().enumerate() {
        if chosen.is_none() && keep(index, &seat) {
            chosen = Some(seat);
        } else if seat.version() >= SEAT_RELEASE_SINCE_VERSION {
            seat.release();
        }
    }
    chosen
}

impl std::fmt::Debug for Display {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Display").finish_non_exhaustive()
    }
}
