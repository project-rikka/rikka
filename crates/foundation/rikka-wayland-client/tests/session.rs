use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};

use rikka_compositor_server::ClientOptions;
use rikka_test_display::Display;
use rikka_wayland_client::wl_callback::{self, WlCallback};
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_data_device::{self, WlDataDevice};
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_data_offer::{self, WlDataOffer};
use rikka_wayland_client::wl_keyboard::{self, WlKeyboard};
use rikka_wayland_client::wl_output::{self, WlOutput};
use rikka_wayland_client::wl_pointer::{self, WlPointer};
use rikka_wayland_client::wl_registry::WlRegistry;
use rikka_wayland_client::wl_seat::{self, WlSeat};
use rikka_wayland_client::wl_shm::{self, WlShm};
use rikka_wayland_client::wl_surface;
use rikka_wayland_client::{Connection, Error, Globals, Queue, Route};

use crate::support::{State, marked_memfd, marker, start};

struct Seen {
    globals: Globals,
    lines: Vec<String>,
    offers: Vec<WlDataOffer>,
}

struct Session {
    connection: Connection,
    queue: Queue<Seen>,
    seen: Seen,
}

impl Session {
    fn open(server: &Display<State>) -> Self {
        Self::over(server.connect(ClientOptions::new()))
    }

    fn over(stream: UnixStream) -> Self {
        let (connection, display) = rikka_wayland_client::from_socket(stream).unwrap();
        let mut queue = connection.queue::<Seen>();
        queue.on::<WlRegistry>(|seen: &mut Seen, _, event| seen.globals.event(event));
        queue.on::<WlCallback>(|seen: &mut Seen, callback, event| {
            if let wl_callback::Event::Done { callback_data } = event {
                seen.lines.push(match callback.key::<u32>() {
                    Some(tag) => format!("done {callback_data} tagged {tag}"),
                    None => format!("done {callback_data}"),
                });
            }
        });
        queue.on::<WlShm>(|seen: &mut Seen, _, event| {
            if let wl_shm::Event::Format { format } = event {
                seen.lines.push(format!("format {format:?}"));
            }
        });
        queue.on::<WlSeat>(|seen: &mut Seen, _, event| match event {
            wl_seat::Event::Capabilities { capabilities } => {
                seen.lines
                    .push(format!("capabilities {}", capabilities.bits()));
            }
            wl_seat::Event::Name { name } => seen.lines.push(format!("seat {name}")),
            _ => {}
        });
        queue.on::<WlKeyboard>(|seen: &mut Seen, _, event| match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                seen.lines
                    .push(format!("keymap {format:?} {size} marked {}", marker(&fd)));
            }
            wl_keyboard::Event::Enter {
                serial,
                surface,
                keys,
            } => seen
                .lines
                .push(format!("keys {serial} on {} {keys:?}", surface.id())),
            _ => {}
        });
        queue.on::<WlPointer>(|seen: &mut Seen, _, event| {
            if let wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            } = event
            {
                seen.lines.push(format!("pointer {surface_x} {surface_y}"));
            }
        });
        queue.on::<WlDataDevice>(|seen: &mut Seen, _, event| match event {
            wl_data_device::Event::DataOffer { id } => seen.offers.push(id),
            wl_data_device::Event::Selection { id } => seen.lines.push(format!(
                "selection {}",
                id.is_some_and(|id| seen.offers.contains(&id))
            )),
            _ => {}
        });
        queue.on::<WlDataOffer>(|seen: &mut Seen, offer, event| {
            if let wl_data_offer::Event::Offer { mime_type } = event {
                seen.lines
                    .push(format!("offer {mime_type} v{}", offer.version()));
            }
        });
        queue.on::<WlOutput>(|seen: &mut Seen, output, event| {
            if let wl_output::Event::Name { name } = event {
                seen.lines
                    .push(format!("output {name} tagged {:?}", output.key::<u32>()));
            }
        });
        let registry = display.via(&queue.route()).get_registry();
        let mut session = Self {
            connection,
            queue,
            seen: Seen {
                globals: Globals::new(registry),
                lines: Vec::new(),
                offers: Vec::new(),
            },
        };
        session.roundtrip();
        session
    }

    fn roundtrip(&mut self) {
        self.queue.roundtrip(&mut self.seen).unwrap();
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.seen.lines)
    }
}

#[test]
fn registry_lists_globals_and_binds_at_the_asked_version() {
    let server = start();
    let mut session = Session::open(&server);
    let mut globals: Vec<(String, u32)> = session
        .seen
        .globals
        .iter()
        .map(|global| (global.interface.to_string(), global.version))
        .collect();
    globals.sort();
    assert_eq!(
        globals,
        [
            ("wl_compositor", 7),
            ("wl_data_device_manager", 4),
            ("wl_fixes", 2),
            ("wl_output", 4),
            ("wl_output", 4),
            ("wl_seat", 7),
            ("wl_shm", 1),
        ]
        .map(|(interface, version)| (interface.to_owned(), version))
    );

    let seat: WlSeat = session.seen.globals.bind(5..=9).unwrap();
    assert_eq!(seat.version(), 7);
    assert!(session.seen.globals.bind::<WlSeat>(8..=9).is_none());
    let outputs: Vec<WlOutput> = session.seen.globals.bind_all(9);
    for (output, tag) in outputs.iter().zip([10u32, 20]) {
        assert_eq!(output.version(), 4);
        output.set_key(tag);
    }
    session.roundtrip();
    assert_eq!(
        session.lines(),
        [
            "capabilities 3",
            "seat seat-test",
            "output DP-1 tagged Some(10)",
            "output HDMI-A-1 tagged Some(20)",
        ]
    );
}

#[test]
fn every_argument_kind_crosses_in_both_directions() {
    let server = start();
    let mut session = Session::open(&server);
    let compositor: WlCompositor = session.seen.globals.bind(1..=6).unwrap();
    let shm: WlShm = session.seen.globals.bind(1..=1).unwrap();
    let seat: WlSeat = session.seen.globals.bind(7..=7).unwrap();
    let devices: WlDataDeviceManager = session.seen.globals.bind(3..=3).unwrap();

    let surface = compositor.create_surface();
    let pool = shm.create_pool(marked_memfd(9), 4096);
    let buffer = pool.create_buffer(0, 4, 4, 16, wl_shm::Format::Argb8888);
    surface.attach(Some(&buffer), -3, 7);
    surface.attach(None, 0, 0);
    surface.set_buffer_scale(2);
    let _frame = surface.frame().with_key(21u32);
    session.roundtrip();
    server.expect(&format!("surface {}", surface.id()));
    server.expect("pool 4096 marked 9");
    server.expect(&format!("attach Some({}) -3 7", buffer.id()));
    server.expect("attach None 0 0");
    server.expect("scale 2");

    let _keyboard = seat.get_keyboard();
    let _pointer = seat.get_pointer();
    let _device = devices.get_data_device(&seat);
    session.roundtrip();
    let offer = session.seen.offers[0].clone();
    offer.accept(11, Some("text/plain"));
    offer.accept(12, None);
    session.roundtrip();
    server.expect("accept 11 Some(\"text/plain\")");
    server.expect("accept 12 None");
    assert_eq!(
        session.lines(),
        [
            "format Ok(Argb8888)".to_owned(),
            "capabilities 3".to_owned(),
            "seat seat-test".to_owned(),
            "done 7 tagged 21".to_owned(),
            "keymap Ok(XkbV1) 1 marked 42".to_owned(),
            format!("keys 3 on {} [30, 0, 0, 0, 31, 0, 0, 0, 9]", surface.id()),
            "pointer 1.5 -2.25".to_owned(),
            "offer text/plain v3".to_owned(),
            "selection true".to_owned(),
            "selection false".to_owned(),
        ]
    );
}

#[test]
fn protocol_error_names_object_code_and_message() {
    let server = start();
    let mut session = Session::open(&server);
    let compositor: WlCompositor = session.seen.globals.bind(6..=6).unwrap();
    let surface = compositor.create_surface();
    surface.set_buffer_scale(0);
    let failure = session.queue.roundtrip(&mut session.seen).unwrap_err();
    let Error::Protocol(error) = failure else {
        panic!("{failure}");
    };
    assert_eq!(error.interface, "wl_surface");
    assert_eq!(error.object, surface.id());
    assert_eq!(error.code, u32::from(wl_surface::Error::InvalidScale));
    assert!(matches!(
        session.connection.error(),
        Some(Error::Protocol(_))
    ));
    assert!(session.queue.roundtrip(&mut session.seen).is_err());
}

#[test]
fn destroyed_ids_return_after_the_server_acknowledges() {
    let server = start();
    let mut session = Session::open(&server);
    let compositor: WlCompositor = session.seen.globals.bind(6..=6).unwrap();
    let first = compositor.create_surface();
    let callback = first.frame();
    first.destroy();
    assert!(!first.alive());
    first.set_buffer_scale(3);
    let second = compositor.create_surface();
    assert_ne!(second.id(), first.id());
    session.roundtrip();
    server.expect(&format!("destroyed surface {}", first.id()));
    assert!(!callback.alive());
    assert_eq!(session.lines(), ["done 7"]);

    let ceiling = second.id() + 2;
    let mut recycled = Vec::new();
    for _ in 0..16 {
        let surface = compositor.create_surface();
        assert!(surface.id() <= ceiling, "{surface:?}");
        assert!(!recycled.contains(&surface));
        surface.destroy();
        session.roundtrip();
        recycled.push(surface);
    }
    assert!(recycled.iter().any(|surface| surface.id() == first.id()));
    assert!(recycled.iter().all(|surface| *surface != first));
}

#[test]
fn routes_send_events_to_other_queues_taps_and_nowhere() {
    let server = start();
    let mut session = Session::open(&server);
    let compositor: WlCompositor = session.seen.globals.bind(6..=6).unwrap();
    let surface = compositor.create_surface();

    let tapped = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&tapped);
    let tap = Route::tap::<WlCallback>(move |_, event| {
        if let wl_callback::Event::Done { callback_data } = event {
            log.lock().unwrap().push(callback_data);
        }
    });
    let _tapped = surface.via(&tap).frame();
    let _discarded = surface.via(&Route::discard()).frame();

    let mut other = session.connection.queue::<Vec<u32>>();
    other.on::<WlCallback>(|done: &mut Vec<u32>, _, event| {
        if let wl_callback::Event::Done { callback_data } = event {
            done.push(callback_data);
        }
    });
    let _elsewhere = surface.via(&other.route()).frame();
    let worker = std::thread::spawn(move || {
        let mut done = Vec::new();
        other.roundtrip(&mut done).unwrap();
        done
    });
    session.roundtrip();
    assert_eq!(worker.join().unwrap(), [7]);
    assert_eq!(*tapped.lock().unwrap(), [7]);
    assert!(session.lines().is_empty());
}
