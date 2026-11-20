use rustix::fd::AsFd;
use std::os::fd::OwnedFd;

use crate::common::{Server, State, Wayland};
use rikka_compositor_surface::{DmabufFeedback, DmabufTranche, TrancheFlags};
use rikka_compositor_types::{Format, Fourcc, Modifier};
use rikka_wayland_client::{zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};

const MAIN: u64 = 0xe280;
const ARGB: u32 = Fourcc::ARGB8888.0;

fn linear(fourcc: Fourcc) -> Format {
    Format::new(fourcc, Modifier::LINEAR)
}

fn sampling() -> DmabufFeedback {
    DmabufFeedback::new(
        MAIN,
        vec![DmabufTranche {
            device: MAIN,
            flags: TrancheFlags::SAMPLING,
            formats: vec![linear(Fourcc::ARGB8888), linear(Fourcc::XRGB8888)],
        }],
    )
    .unwrap()
}

fn enabled(state: &mut State) {
    let display = state.display.clone();
    state.surfaces.enable_dmabuf(&display, sampling());
}

fn memory(size: u64) -> OwnedFd {
    let fd = memfd_create("dmabuf-test", MemfdFlags::CLOEXEC).unwrap();
    ftruncate(&fd, size).unwrap();
    fd
}

fn params(
    _client: &Wayland,
    dmabuf: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    fd: &OwnedFd,
    stride: u32,
) -> zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1 {
    let params = dmabuf.create_params();
    params.add(fd.as_fd(), 0, 0, stride, 0, 0);
    params
}

#[test]
fn feedback_sends_tranches_and_sealed_table() {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = client.global(6);
    dmabuf.get_default_feedback();
    client.roundtrip();
    assert_eq!(
        client.received.dmabuf,
        vec![
            "table 32".to_owned(),
            format!("target {MAIN}"),
            "flags 2".to_owned(),
            "formats [0, 1]".to_owned(),
            "tranche done".to_owned(),
            "done".to_owned(),
        ]
    );
    let (table, size) = &client.received.tables[0];
    let mut bytes = vec![0u8; *size as usize];
    rustix::io::pread(table, &mut bytes, 0).unwrap();
    assert_eq!(u32::from_ne_bytes(bytes[0..4].try_into().unwrap()), ARGB);
    assert_eq!(u64::from_ne_bytes(bytes[8..16].try_into().unwrap()), 0);
    assert!(rustix::io::write(table, &[1]).is_err());

    let old: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = client.global(5);
    client.received.dmabuf.clear();
    old.get_default_feedback();
    client.roundtrip();
    assert_eq!(client.received.dmabuf[1], format!("main {MAIN}"));
    assert_eq!(client.received.dmabuf[3], "flags 0");
}

#[test]
fn dmabuf_commit_waits_for_fence() {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = client.global(6);
    let (fence, signal) = rustix::pipe::pipe().unwrap();
    let params = params(&client, &dmabuf, &fence, 64);
    let buffer = params.create_immed(16, 32, ARGB, zwp_linux_buffer_params_v1::Flags::empty());
    surface.attach(Some(&buffer), 0, 0);
    surface.commit();
    client.roundtrip();
    let id = surface.id();
    let applied = server.call(move |state| state.commits_of(state.surface(id)).len());
    assert_eq!(applied, 0);
    rustix::io::write(&signal, &[1]).unwrap();
    server.await_commits(id, 1);
}

fn rejected(configure: impl FnOnce(&Wayland, &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1), code: u32) {
    let server = Server::with(enabled);
    let mut client = server.connect();
    let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = client.global(6);
    configure(&client, &dmabuf);
    assert!(client.fails());
    server.expect(&format!(
        "protocol error {code} on zwp_linux_buffer_params_v1"
    ));
}

#[test]
fn params_errors_are_protocol_errors() {
    let fd = memory(64 * 32);
    let flags = zwp_linux_buffer_params_v1::Flags::empty();
    rejected(
        |client, dmabuf| {
            let params = params(client, dmabuf, &fd, 64);
            params.create(16, 32, ARGB, flags);
            params.create(16, 32, ARGB, flags);
        },
        0,
    );
    rejected(
        |_client, dmabuf| {
            dmabuf.create_params().add(fd.as_fd(), 4, 0, 64, 0, 0);
        },
        1,
    );
    rejected(
        |client, dmabuf| params(client, dmabuf, &fd, 64).add(fd.as_fd(), 0, 0, 64, 0, 0),
        2,
    );
    rejected(
        |_client, dmabuf| {
            let params = dmabuf.create_params();
            params.add(fd.as_fd(), 1, 0, 64, 0, 0);
            params.create(16, 32, ARGB, flags);
        },
        3,
    );
    rejected(
        |client, dmabuf| params(client, dmabuf, &fd, 64).create(16, 32, Fourcc::ABGR8888.0, flags),
        4,
    );
    rejected(
        |_client, dmabuf| {
            let params = dmabuf.create_params();
            params.add(fd.as_fd(), 0, 0, 64, 0, 0);
            params.add(fd.as_fd(), 1, 0, 64, 0, 1);
        },
        4,
    );
    rejected(
        |client, dmabuf| params(client, dmabuf, &fd, 64).create(0, 32, ARGB, flags),
        5,
    );
    rejected(
        |client, dmabuf| params(client, dmabuf, &fd, 64).create(16, 64, ARGB, flags),
        6,
    );
    rejected(
        |client, dmabuf| params(client, dmabuf, &fd, 64).set_sampling_device(&[0; 4]),
        8,
    );
}
