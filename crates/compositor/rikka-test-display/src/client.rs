use std::fmt::{self, Debug};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rikka_sync::Mutex;
use rikka_wayland_client::wl_display::WlDisplay;
use rikka_wayland_client::wl_registry::{self, WlRegistry};
use rikka_wayland_client::wl_shm;
use rikka_wayland_client::{Connection, Error, Global, ProtocolError, Proxy, Queue, Route};

use crate::canvas::Canvas;
use crate::display::PATIENCE;
use crate::wait;

pub struct Client<R> {
    pub connection: Connection,
    pub queue: Queue<R>,
    pub received: R,
    display: WlDisplay,
    registry: WlRegistry,
    globals: Arc<Mutex<Vec<Global>>>,
    patience: Duration,
}

impl<R: Default> Client<R> {
    pub fn connect(stream: UnixStream, listen: impl FnOnce(&mut Queue<R>)) -> Self {
        let connected =
            rikka_wayland_client::from_socket(stream).expect("a client over the socket");
        Self::over(connected, listen)
    }

    pub fn connect_to(socket: &Path, listen: impl FnOnce(&mut Queue<R>)) -> Self {
        let connected = rikka_wayland_client::connect_to(socket).expect("a client of the socket");
        Self::over(connected, listen)
    }

    fn over(
        (connection, display): (Connection, WlDisplay),
        listen: impl FnOnce(&mut Queue<R>),
    ) -> Self {
        let mut queue = connection.queue();
        listen(&mut queue);
        let globals = Arc::new(Mutex::new(Vec::new()));
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
        let mut client = Self {
            connection,
            queue,
            received: R::default(),
            display,
            registry,
            globals,
            patience: PATIENCE,
        };
        client.roundtrip();
        client
    }
}

impl<R: Debug> Client<R> {
    pub fn until(&mut self, what: &str, done: impl Fn(&R) -> bool) {
        wait::dispatch_until(
            &mut self.queue,
            &mut self.received,
            what,
            self.patience,
            done,
        );
    }
}

impl<R> Client<R> {
    #[must_use]
    pub fn with_patience(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    pub fn roundtrip(&mut self) {
        self.queue
            .roundtrip(&mut self.received)
            .expect("the display answers a roundtrip");
    }

    pub fn fails(&mut self) -> bool {
        self.queue.roundtrip(&mut self.received).is_err()
    }

    pub fn protocol_error(&mut self) -> Option<ProtocolError> {
        match self.queue.roundtrip(&mut self.received) {
            Err(Error::Protocol(error)) => Some(error),
            _ => None,
        }
    }

    pub fn display(&self) -> WlDisplay {
        self.display.via(&self.queue.route())
    }

    pub fn registry(&self) -> &WlRegistry {
        &self.registry
    }

    pub fn route(&self) -> Route {
        self.queue.route()
    }

    pub fn globals(&self) -> Vec<Global> {
        self.globals.lock().clone()
    }

    pub fn advertises(&self, interface: &str) -> bool {
        self.globals()
            .iter()
            .any(|global| &*global.interface == interface)
    }

    pub fn global<P: Proxy>(&self, version: u32) -> P {
        let name = P::INTERFACE.name;
        let global = self
            .globals()
            .into_iter()
            .find(|global| &*global.interface == name)
            .unwrap_or_else(|| panic!("{name} is not advertised"));
        self.bind(global.name, version.min(global.version))
    }

    pub fn bind<P: Proxy>(&self, name: u32, version: u32) -> P {
        self.registry.via(&self.queue.route()).bind(name, version)
    }

    pub fn canvas(&self, width: i32, height: i32, format: wl_shm::Format) -> Canvas {
        Canvas::new(&self.global(1), width, height, format)
    }
}

impl<R> fmt::Debug for Client<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Client")
            .field("connection", &self.connection)
            .finish_non_exhaustive()
    }
}
