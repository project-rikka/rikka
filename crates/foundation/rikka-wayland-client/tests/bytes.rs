use std::io::{IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_data_device::{self, WlDataDevice};
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_data_offer::WlDataOffer;
use rikka_wayland_client::wl_keyboard::{self, WlKeyboard};
use rikka_wayland_client::wl_output::{self, WlOutput};
use rikka_wayland_client::wl_pointer::{self, WlPointer};
use rikka_wayland_client::wl_seat::WlSeat;
use rikka_wayland_client::wl_shm::{self, WlShm};
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_presentation::WpPresentation;
use rikka_wayland_client::wp_presentation_feedback::{self, WpPresentationFeedback};
use rikka_wayland_client::wp_viewporter::WpViewporter;
use rikka_wayland_client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use rikka_wayland_client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use rikka_wayland_client::zwp_text_input_v3::{self, ZwpTextInputV3};
use rikka_wayland_client::{Connection, Queue, UnknownValue};
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};

use crate::support::{marked_memfd, marker};

const SERVER_ID: u32 = 0xFF00_0000;

enum Part<'a> {
    Words(&'a [u32]),
    Bytes(&'a [u8]),
}
use Part::{Bytes, Words};

fn message(object: u32, opcode: u16, parts: &[Part<'_>]) -> Vec<u8> {
    let mut body = Vec::new();
    for part in parts {
        match part {
            Words(words) => {
                for word in *words {
                    body.extend_from_slice(&word.to_ne_bytes());
                }
            }
            Bytes(bytes) => body.extend_from_slice(bytes),
        }
    }
    assert_eq!(body.len() % 4, 0, "fixtures spell their padding out");
    let size = (body.len() + 8) as u32;
    let mut bytes = object.to_ne_bytes().to_vec();
    bytes.extend_from_slice(&((size << 16) | u32::from(opcode)).to_ne_bytes());
    bytes.extend_from_slice(&body);
    bytes
}

struct Peer(UnixStream);

impl Peer {
    fn receive(&self) -> (Vec<u8>, Vec<OwnedFd>) {
        let mut bytes = vec![0u8; 4096];
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let received = recvmsg(
            &self.0,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut control,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
        )
        .unwrap();
        bytes.truncate(received.bytes);
        let fds = control
            .drain()
            .filter_map(|message| match message {
                RecvAncillaryMessage::ScmRights(fds) => Some(fds),
                _ => None,
            })
            .flatten()
            .collect();
        (bytes, fds)
    }

    fn send(&self, bytes: &[u8], fds: &[OwnedFd]) {
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        let borrowed: Vec<_> = fds.iter().map(std::os::fd::AsFd::as_fd).collect();
        if !borrowed.is_empty() {
            control.push(SendAncillaryMessage::ScmRights(&borrowed));
        }
        let sent = sendmsg(
            &self.0,
            &[IoSlice::new(bytes)],
            &mut control,
            SendFlags::empty(),
        );
        assert_eq!(sent.unwrap(), bytes.len());
    }
}

struct Wired {
    connection: Connection,
    queue: Queue<Vec<String>>,
    peer: Peer,
    registry: rikka_wayland_client::wl_registry::WlRegistry,
}

impl Wired {
    fn new() -> Self {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let (connection, display) = rikka_wayland_client::from_socket(ours).unwrap();
        let queue = connection.queue::<Vec<String>>();
        let registry = display.via(&queue.route()).get_registry();
        let wired = Self {
            connection,
            queue,
            peer: Peer(theirs),
            registry,
        };
        assert_eq!(wired.sent().0, message(1, 1, &[Words(&[2])]));
        wired
    }

    fn sent(&self) -> (Vec<u8>, Vec<OwnedFd>) {
        self.connection.flush().unwrap();
        self.peer.receive()
    }

    fn arrive(&mut self, bytes: &[u8], fds: &[OwnedFd]) -> Vec<String> {
        self.peer.send(bytes, fds);
        self.connection.read().unwrap();
        let mut seen = Vec::new();
        self.queue.dispatch_pending(&mut seen).unwrap();
        seen
    }
}

#[test]
fn requests_encode_to_golden_bytes() {
    let wired = Wired::new();

    let compositor: WlCompositor = wired.registry.bind(7, 6);
    assert_eq!(
        wired.sent().0,
        message(
            2,
            0,
            &[
                Words(&[7, 14]),
                Bytes(b"wl_compositor\0\0\0"),
                Words(&[6, 3])
            ]
        ),
        "uint, untyped new_id"
    );

    let surface = compositor.create_surface();
    assert_eq!(wired.sent().0, message(3, 0, &[Words(&[4])]), "new_id");

    surface.set_buffer_scale(-2);
    assert_eq!(
        wired.sent().0,
        message(4, 8, &[Words(&[-2i32 as u32])]),
        "int"
    );

    let shm: WlShm = wired.registry.bind(8, 1);
    wired.sent();
    let pool = shm.create_pool(marked_memfd(77), 4096);
    let (bytes, fds) = wired.sent();
    assert_eq!(bytes, message(5, 0, &[Words(&[6, 4096])]), "fd");
    assert_eq!(fds.iter().map(marker).collect::<Vec<_>>(), [77]);

    let buffer = pool.create_buffer(0, 2, 2, 8, wl_shm::Format::Xrgb8888);
    assert_eq!(
        wired.sent().0,
        message(6, 0, &[Words(&[7, 0, 2, 2, 8, 1])]),
        "enum"
    );
    surface.attach(Some(&buffer), 1, -1);
    surface.attach(None, 0, 0);
    let mut expected = message(4, 1, &[Words(&[7, 1, -1i32 as u32])]);
    expected.extend(message(4, 1, &[Words(&[0, 0, 0])]));
    assert_eq!(wired.sent().0, expected, "object, nullable object");

    let viewporter: WpViewporter = wired.registry.bind(9, 1);
    let viewport = viewporter.get_viewport(&surface);
    wired.sent();
    viewport.set_source(1.5, -0.25, 640.0, 0.00390625);
    assert_eq!(
        wired.sent().0,
        message(9, 1, &[Words(&[384, -64i32 as u32, 163_840, 1])]),
        "fixed"
    );

    let devices: WlDataDeviceManager = wired.registry.bind(10, 3);
    let source = devices.create_data_source();
    wired.sent();
    source.offer("text/plain;charset=utf-8");
    source.offer("abc");
    let mut expected = message(
        11,
        0,
        &[Words(&[25]), Bytes(b"text/plain;charset=utf-8\0\0\0\0")],
    );
    expected.extend(message(11, 0, &[Words(&[4]), Bytes(b"abc\0")]));
    assert_eq!(wired.sent().0, expected, "string");

    let dmabuf: ZwpLinuxDmabufV1 = wired.registry.bind(11, 6);
    let params = dmabuf.create_params();
    wired.sent();
    params.set_sampling_device(&[1, 2, 3, 4, 5]);
    params.set_sampling_device(&[]);
    let mut expected = message(13, 4, &[Words(&[5]), Bytes(&[1, 2, 3, 4, 5, 0, 0, 0])]);
    expected.extend(message(13, 4, &[Words(&[0])]));
    assert_eq!(wired.sent().0, expected, "array");

    surface.destroy();
    surface.set_buffer_scale(1);
    assert_eq!(
        wired.sent().0,
        message(4, 0, &[]),
        "destructor, then nothing"
    );
}

#[test]
fn golden_bytes_decode_to_events() {
    let mut wired = Wired::new();
    wired
        .queue
        .on::<WlOutput>(|seen: &mut Vec<String>, _, event| {
            if let wl_output::Event::Geometry {
                x,
                y,
                subpixel,
                make,
                model,
                ..
            } = event
            {
                seen.push(format!("{x} {y} {subpixel:?} {make:?} {model:?}"));
            }
        });
    wired
        .queue
        .on::<WlPointer>(|seen: &mut Vec<String>, _, event| {
            if let wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } = event
            {
                seen.push(format!("{time} {surface_x} {surface_y}"));
            }
        });
    wired
        .queue
        .on::<WlKeyboard>(|seen: &mut Vec<String>, _, event| match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                seen.push(format!("{format:?} {} {size}", marker(&fd)));
            }
            wl_keyboard::Event::Enter {
                serial,
                surface,
                keys,
            } => seen.push(format!("{serial} {} {keys:?}", surface.id())),
            _ => {}
        });
    wired
        .queue
        .on::<WlDataDevice>(|seen: &mut Vec<String>, _, event| match event {
            wl_data_device::Event::DataOffer { id } => {
                id.accept(5, None);
                id.accept(6, Some("a"));
                seen.push(format!("offer {:#x} v{}", id.id(), id.version()));
            }
            wl_data_device::Event::Selection { id } => {
                seen.push(format!("selection {:?}", id.map(|id| id.id())));
            }
            _ => {}
        });
    wired
        .queue
        .on::<ZwpTextInputV3>(|seen: &mut Vec<String>, _, event| {
            if let zwp_text_input_v3::Event::PreeditString { text, .. } = event {
                seen.push(format!("{text:?}"));
            }
        });
    wired.queue.on::<WlDataOffer>(|_, _, _| {});

    let output: WlOutput = wired.registry.bind(1, 4);
    let seat: WlSeat = wired.registry.bind(2, 7);
    let compositor: WlCompositor = wired.registry.bind(3, 6);
    let devices: WlDataDeviceManager = wired.registry.bind(4, 3);
    let inputs: ZwpTextInputManagerV3 = wired.registry.bind(5, 1);
    let pointer = seat.get_pointer();
    let keyboard = seat.get_keyboard();
    let surface: WlSurface = compositor.create_surface();
    let device = devices.get_data_device(&seat);
    let input = inputs.get_text_input(&seat);
    wired.sent();
    assert_eq!(
        [
            output.id(),
            pointer.id(),
            keyboard.id(),
            surface.id(),
            device.id(),
            input.id()
        ],
        [3, 8, 9, 10, 11, 12]
    );

    let geometry = message(
        3,
        0,
        &[
            Words(&[-5i32 as u32, 7, 300, 200, 2, 6]),
            Bytes(b"Mak\xC3\xA9\0\0\0"),
            Words(&[1]),
            Bytes(b"\0\0\0\0"),
            Words(&[0]),
        ],
    );
    assert_eq!(
        wired.arrive(&geometry, &[]),
        ["-5 7 Ok(HorizontalRgb) \"Maké\" \"\""],
        "int, enum, string"
    );

    let motion = message(8, 2, &[Words(&[99, 384, -64i32 as u32])]);
    assert_eq!(wired.arrive(&motion, &[]), ["99 1.5 -0.25"], "uint, fixed");

    let keymap = message(9, 0, &[Words(&[1, 33])]);
    let unknown = message(9, 0, &[Words(&[99, 0])]);
    assert_eq!(
        wired.arrive(
            &[keymap, unknown].concat(),
            &[marked_memfd(5), marked_memfd(6)]
        ),
        [
            "Ok(XkbV1) 5 33".to_owned(),
            format!("{:?} 6 0", Err::<(), _>(UnknownValue(99))),
        ],
        "fd, unknown enum value"
    );

    let enter = message(
        9,
        1,
        &[Words(&[4, 10, 6]), Bytes(&[1, 0, 0, 0, 2, 0, 0, 0])],
    );
    assert_eq!(
        wired.arrive(&enter, &[]),
        ["4 10 [1, 0, 0, 0, 2, 0]"],
        "object, array"
    );

    let offered = message(11, 0, &[Words(&[SERVER_ID])]);
    let selected = message(11, 5, &[Words(&[SERVER_ID])]);
    let cleared = message(11, 5, &[Words(&[0])]);
    assert_eq!(
        wired.arrive(&[offered, selected, cleared].concat(), &[]),
        [
            "offer 0xff000000 v3".to_owned(),
            format!("selection Some({SERVER_ID})"),
            "selection None".to_owned(),
        ],
        "new_id, nullable object"
    );
    let mut accepted = message(SERVER_ID, 0, &[Words(&[5, 0])]);
    accepted.extend(message(SERVER_ID, 0, &[Words(&[6, 2]), Bytes(b"a\0\0\0")]));
    assert_eq!(wired.sent().0, accepted, "nullable string request");

    let empty = message(12, 2, &[Words(&[0, 0, 0])]);
    let filled = message(12, 2, &[Words(&[3]), Bytes(b"ab\0\0"), Words(&[1, 2])]);
    assert_eq!(
        wired.arrive(&[empty, filled].concat(), &[]),
        ["None", "Some(\"ab\")"],
        "nullable string"
    );
}

#[test]
fn events_queued_before_a_destructor_keep_their_key() {
    let mut wired = Wired::new();
    wired
        .queue
        .on::<WpPresentationFeedback>(|seen: &mut Vec<String>, feedback, event| {
            let name = match event {
                wp_presentation_feedback::Event::SyncOutput { .. } => "sync_output",
                wp_presentation_feedback::Event::Presented { .. } => "presented",
                _ => "other",
            };
            seen.push(format!("{:?} {name}", feedback.key::<u32>()));
        });
    let compositor: WlCompositor = wired.registry.bind(1, 4);
    let presentation: WpPresentation = wired.registry.bind(2, 1);
    let output: WlOutput = wired.registry.bind(3, 4);
    let surface = compositor.create_surface();
    let feedback = presentation.feedback(&surface).with_key(9u32);
    wired.sent();

    let synced = message(feedback.id(), 0, &[Words(&[output.id()])]);
    let presented = message(feedback.id(), 1, &[Words(&[0; 7])]);
    let deleted = message(1, 1, &[Words(&[feedback.id()])]);
    assert_eq!(
        wired.arrive(&[synced, presented, deleted].concat(), &[]),
        ["Some(9) sync_output", "Some(9) presented"],
    );
}
