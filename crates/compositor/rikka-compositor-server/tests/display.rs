use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;

use rikka_test_display::Client;

use crate::common::{Counter, RawClient, Server, State, Tag, serve};
use rikka_compositor_protocols as protocols;
use rikka_compositor_server::{ClientOptions, SecurityContext};
use rikka_wayland_client::{wl_callback, wl_fixes, wl_shm};
use rikka_wayland_wire::{DISPLAY_ID, MAX_FDS_PER_MESSAGE};
use rustix::fs::{MemfdFlags, memfd_create};

pub(crate) fn standard(state: &mut State) {
    let display = state.display.clone();
    serve(&display);
    let globals = [
        (
            "wl_compositor",
            display
                .global::<protocols::wl_compositor::WlCompositor>()
                .version(6)
                .publish(),
        ),
        (
            "wl_output",
            display
                .global::<protocols::wl_output::WlOutput>()
                .version(4)
                .key(Tag("DP-1"))
                .publish(),
        ),
        (
            "wl_shm",
            display
                .global::<protocols::wl_shm::WlShm>()
                .version(1)
                .publish(),
        ),
        (
            "wl_seat",
            display
                .global::<protocols::wl_seat::WlSeat>()
                .version(7)
                .privileged()
                .publish(),
        ),
        (
            "wl_data_device_manager",
            display
                .global::<protocols::wl_data_device_manager::WlDataDeviceManager>()
                .version(3)
                .publish(),
        ),
    ];
    state.globals.extend(globals);
    display.install(Counter);
    display.on_disconnect(|state, client, reason| {
        state.note(format!("client {} left: {reason}", client.id().raw()));
    });
}

#[derive(Default)]
struct Received {
    done: Vec<u32>,
}

type Wayland = Client<Received>;

fn connect(stream: UnixStream) -> Wayland {
    Client::connect(stream, |queue| {
        queue.on::<wl_callback::WlCallback>(|received: &mut Received, _, event| {
            if let wl_callback::Event::Done { callback_data } = event {
                received.done.push(callback_data);
            }
        });
    })
}

fn interfaces(client: &Wayland) -> Vec<String> {
    let mut interfaces: Vec<String> = client
        .globals()
        .into_iter()
        .map(|global| global.interface.into())
        .collect();
    interfaces.sort_unstable();
    interfaces
}

fn name(client: &Wayland, interface: &str) -> u32 {
    client
        .globals()
        .into_iter()
        .find(|global| &*global.interface == interface)
        .map(|global| global.name)
        .unwrap_or_else(|| panic!("{interface} is not advertised"))
}

pub(crate) fn marked_memfd(marker: u8) -> OwnedFd {
    let fd = memfd_create("display-test", MemfdFlags::CLOEXEC).unwrap();
    rustix::io::write(&fd, &[marker]).unwrap();
    fd
}

#[test]
fn filtered_globals_hidden_from_sandboxed() {
    let server = Server::start(standard);
    let trusted = connect(server.connect(ClientOptions::new()));
    let sandbox = || {
        ClientOptions::new().security_context(SecurityContext {
            sandbox_engine: Some("flatpak".into()),
            app_id: Some("org.example.App".into()),
            instance_id: Some("7".into()),
        })
    };
    let sandboxed = connect(server.connect(sandbox()));

    assert_eq!(
        interfaces(&trusted),
        [
            "wl_compositor",
            "wl_data_device_manager",
            "wl_fixes",
            "wl_output",
            "wl_seat",
            "wl_shm",
            "wl_subcompositor",
        ]
    );
    assert!(!sandboxed.advertises("wl_seat"));
    assert!(sandboxed.advertises("wl_compositor"));

    let clients = server.call(|state| {
        state
            .display
            .clients()
            .iter()
            .map(|client| {
                (
                    client.privileged(),
                    client
                        .security_context()
                        .and_then(|context| context.app_id.clone()),
                )
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(
        clients,
        [(true, None), (false, Some("org.example.App".to_owned()))]
    );

    let hidden = name(&trusted, "wl_seat");
    let mut intruder = RawClient::new(server.connect(sandbox()));
    let (registry, _) = intruder.registry();
    intruder.bind(registry, hidden, "wl_seat", 1);
    assert_eq!(
        intruder.error(),
        (
            registry,
            0,
            format!("global wl_seat ({hidden}) is unavailable")
        )
    );
}

#[test]
fn binding_above_version_is_protocol_error() {
    let server = Server::start(standard);
    let mut raw = RawClient::new(server.connect(ClientOptions::new()));
    let (registry, globals) = raw.registry();
    let (name, _, version) = globals
        .into_iter()
        .find(|(_, interface, _)| interface == "wl_output")
        .unwrap();
    raw.bind(registry, name, "wl_output", version + 1);
    assert_eq!(
        raw.error(),
        (
            registry,
            0,
            format!("invalid version for global wl_output ({name}): expected at most 4, got 5")
        )
    );
    raw.await_hangup();
}

#[test]
fn request_above_version_is_protocol_error() {
    let server = Server::start(standard);
    let mut raw = RawClient::new(server.connect(ClientOptions::new()));
    let (registry, globals) = raw.registry();
    let (name, _, _) = globals
        .into_iter()
        .find(|(_, interface, _)| interface == "wl_output")
        .unwrap();
    let output = raw.bind(registry, name, "wl_output", 2);
    raw.send(output, 0, |_| {});
    assert_eq!(
        raw.error(),
        (
            DISPLAY_ID,
            1,
            format!("invalid version for wl_output#{output}.release (2, need at least 3)")
        )
    );
}

#[test]
fn malformed_request_disconnects_sender_only() {
    struct Offence {
        commit: fn(&mut RawClient),
        code: u32,
        message: &'static str,
    }

    let server = Server::start(standard);
    let mut bystander = connect(server.connect(ClientOptions::new()));
    let offences = [
        Offence {
            commit: |raw| raw.send(77, 0, |_| {}),
            code: 0,
            message: "invalid object 77",
        },
        Offence {
            commit: |raw| raw.send(DISPLAY_ID, 9, |_| {}),
            code: 1,
            message: "invalid method 9, object wl_display#1",
        },
        Offence {
            commit: |raw| raw.send_bytes(&[1, 0, 0, 0, 0, 0, 4, 0]),
            code: 1,
            message: "message size 4 is not a multiple of four between 8 and 4096 bytes",
        },
        Offence {
            commit: |raw| raw.send(DISPLAY_ID, 0, |_| {}),
            code: 1,
            message: "invalid arguments for wl_display#1.sync: message truncated before its last argument",
        },
    ];
    for offence in offences {
        let mut raw = RawClient::new(server.connect(ClientOptions::new()));
        raw.roundtrip();
        (offence.commit)(&mut raw);
        assert_eq!(
            raw.error(),
            (DISPLAY_ID, offence.code, offence.message.to_owned())
        );
        raw.await_hangup();
        let _synced = bystander.display().sync();
        bystander.roundtrip();
    }
    assert_eq!(bystander.received.done.len(), 4);
    assert_eq!(server.call(|state| state.display.clients().len()), 1);
}

#[test]
fn acknowledged_removal_withdraws_global() {
    let server = Server::start(standard);
    let mut client = connect(server.connect(ClientOptions::new()));
    let fixes: wl_fixes::WlFixes = client.global(2);
    let name = name(&client, "wl_output");
    server.call(|state| state.globals["wl_output"].remove());
    client.roundtrip();
    assert!(!client.advertises("wl_output"));
    assert!(server.call(|state| state.globals["wl_output"].bindable()));

    fixes.ack_global_remove(client.registry(), name);
    client.roundtrip();
    assert!(!server.call(|state| state.globals["wl_output"].bindable()));

    let mut raw = RawClient::new(server.connect(ClientOptions::new()));
    let (registry, globals) = raw.registry();
    let find = |name: &str| {
        globals
            .iter()
            .find(|(_, interface, _)| interface == name)
            .map(|(name, _, _)| *name)
            .unwrap()
    };
    let compositor = find("wl_compositor");
    let fixes = raw.bind(registry, find("wl_fixes"), "wl_fixes", 2);
    raw.send(fixes, 2, |writer| {
        writer.object_id(registry).uint(compositor);
    });
    assert_eq!(
        raw.error(),
        (fixes, 0, format!("global {compositor} is not removed"))
    );

    let mut stranger = RawClient::new(server.connect(ClientOptions::new()));
    let (registry, _) = stranger.registry();
    let fixes = stranger.bind(registry, find("wl_fixes"), "wl_fixes", 2);
    stranger.send(fixes, 2, |writer| {
        writer.object_id(registry).uint(name);
    });
    assert_eq!(
        stranger.error(),
        (fixes, 0, format!("registry did not announce global {name}"))
    );
}

#[test]
fn fds_reach_handlers() {
    let server = Server::start(standard);
    let mut client = connect(server.connect(ClientOptions::new()));
    let shm: wl_shm::WlShm = client.global(1);
    let fd = marked_memfd(9);
    let pool = shm.create_pool(fd.as_fd(), 4096);
    client.roundtrip();
    server.expect(&format!("pool#{} of 4096 marked 9", pool.id()));
}

#[test]
fn fd_overflow_disconnects_client() {
    let server = Server::start(standard);
    let mut raw = RawClient::new(server.connect(ClientOptions::new()));
    raw.roundtrip();
    let fds: Vec<OwnedFd> = (0..=MAX_FDS_PER_MESSAGE as u8).map(marked_memfd).collect();
    let borrowed: Vec<_> = fds.iter().map(|fd| fd.as_fd()).collect();
    raw.send_fds(&borrowed);
    assert_eq!(
        raw.error(),
        (
            DISPLAY_ID,
            1,
            "more than 28 file descriptors in one message".to_owned()
        )
    );
    raw.await_hangup();
}

#[test]
fn disconnect_destroys_objects() {
    let server = Server::start(standard);
    let mut raw = RawClient::new(server.connect(ClientOptions::new()));
    let (registry, globals) = raw.registry();
    let (name, _, _) = globals
        .into_iter()
        .find(|(_, interface, _)| interface == "wl_compositor")
        .unwrap();
    let compositor = raw.bind(registry, name, "wl_compositor", 6);
    let surfaces = [raw.allocate(), raw.allocate()];
    for surface in surfaces {
        raw.send(compositor, 0, |writer| {
            writer.object_id(surface);
        });
    }
    let region = raw.allocate();
    raw.send(compositor, 1, |writer| {
        writer.object_id(region);
    });
    raw.roundtrip();
    drop(raw);

    server.expect("destroyed wl_compositor");
    server.expect(&format!("destroyed wl_surface#{} alive false", surfaces[0]));
    server.expect(&format!("destroyed wl_surface#{} alive false", surfaces[1]));
    server.expect(&format!("destroyed wl_region#{region}"));
    assert_eq!(server.call(|state| state.display.clients().len()), 0);
}
