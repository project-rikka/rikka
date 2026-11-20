use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use crate::common::Server;
use crate::display::{marked_memfd, standard};
use rikka_compositor_server::ClientOptions;
use wayland_client::protocol::{
    wl_callback, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer, wl_fixes,
    wl_output, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_subcompositor, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};

#[derive(Default)]
struct Received {
    globals: Vec<(&'static str, u32, String, u32)>,
    removed: Vec<u32>,
    done: Vec<u32>,
    output_names: Vec<String>,
    mime_types: Vec<String>,
    offer: Option<wl_data_offer::WlDataOffer>,
    selection: bool,
}

impl Dispatch<wl_registry::WlRegistry, &'static str> for Received {
    fn event(
        received: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        label: &&'static str,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => received.globals.push((label, name, interface, version)),
            wl_registry::Event::GlobalRemove { name } => received.removed.push(name),
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Received {
    fn event(
        received: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data } = event {
            received.done.push(callback_data);
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Received {
    fn event(
        received: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            received.output_names.push(name);
        }
    }
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for Received {
    fn event(
        received: &mut Self,
        _: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { id } => received.offer = Some(id),
            wl_data_device::Event::Selection { id } => received.selection = id.is_some(),
            _ => {}
        }
    }

    wayland_client::event_created_child!(Received, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wl_data_offer::WlDataOffer, ()),
    ]);
}

impl Dispatch<wl_data_offer::WlDataOffer, ()> for Received {
    fn event(
        received: &mut Self,
        _: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            received.mime_types.push(mime_type);
        }
    }
}

wayland_client::delegate_noop!(Received: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(Received: ignore wl_surface::WlSurface);
wayland_client::delegate_noop!(Received: ignore wl_region::WlRegion);
wayland_client::delegate_noop!(Received: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(Received: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(Received: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(Received: ignore wl_data_device_manager::WlDataDeviceManager);
wayland_client::delegate_noop!(Received: ignore wl_fixes::WlFixes);
wayland_client::delegate_noop!(Received: ignore wl_subcompositor::WlSubcompositor);

struct Wayland {
    connection: Connection,
    queue: EventQueue<Received>,
    handle: QueueHandle<Received>,
    registry: wl_registry::WlRegistry,
    received: Received,
}

impl Wayland {
    fn new(stream: UnixStream) -> Self {
        let connection = Connection::from_socket(stream).unwrap();
        let queue = connection.new_event_queue();
        let handle = queue.handle();
        let registry = connection.display().get_registry(&handle, "first");
        let mut wayland = Self {
            connection,
            queue,
            handle,
            registry,
            received: Received::default(),
        };
        wayland.roundtrip();
        wayland
    }

    fn roundtrip(&mut self) {
        self.queue.roundtrip(&mut self.received).unwrap();
    }

    fn name(&self, interface: &str) -> u32 {
        self.received
            .globals
            .iter()
            .find(|(_, _, candidate, _)| candidate == interface)
            .map(|(_, name, _, _)| *name)
            .unwrap_or_else(|| panic!("{interface} is not advertised"))
    }

    fn bind<I>(&self, version: u32) -> I
    where
        I: Proxy + 'static,
        Received: Dispatch<I, ()>,
    {
        self.registry
            .bind::<I, _, _>(self.name(I::interface().name), version, &self.handle, ())
    }
}

#[test]
fn libwayland_client_exchanges_every_argument_kind() {
    let server = Server::start(standard);
    let mut client = Wayland::new(server.connect(ClientOptions::new()));
    let advertised: Vec<(&str, u32)> = {
        let mut globals: Vec<_> = client
            .received
            .globals
            .iter()
            .map(|(_, _, interface, version)| (interface.as_str(), *version))
            .collect();
        globals.sort_unstable();
        globals
    };
    assert_eq!(
        advertised,
        [
            ("wl_compositor", 6),
            ("wl_data_device_manager", 3),
            ("wl_fixes", 2),
            ("wl_output", 4),
            ("wl_seat", 7),
            ("wl_shm", 1),
            ("wl_subcompositor", 1),
        ]
    );

    let shm: wl_shm::WlShm = client.bind(1);
    let pool = shm.create_pool(marked_memfd(9).as_fd(), 4096, &client.handle, ());
    let output: wl_output::WlOutput = client.bind(4);
    let seat: wl_seat::WlSeat = client.bind(7);
    let devices: wl_data_device_manager::WlDataDeviceManager = client.bind(3);
    let _device = devices.get_data_device(&seat, &client.handle, ());
    client.connection.display().sync(&client.handle, ());
    client.roundtrip();
    server.expect(&format!(
        "pool#{} of 4096 marked 9",
        pool.id().protocol_id()
    ));
    server.expect("bound wl_output v4 DP-1");
    assert_eq!(client.received.output_names, ["DP-1"]);
    assert_eq!(client.received.mime_types, ["text/plain"]);
    assert!(client.received.selection);
    assert_eq!(client.received.done.len(), 1);

    let offer = client.received.offer.take().unwrap();
    assert!(offer.id().protocol_id() >= 0xFF00_0000);
    offer.receive("text/plain".to_owned(), marked_memfd(3).as_fd());
    offer.destroy();
    output.release();
    client.roundtrip();
    server.expect("receive text/plain from clipboard");
    server.expect("destroyed wl_data_offer clipboard");
    server.expect(&format!("released wl_output#{}", output.id().protocol_id()));
}
