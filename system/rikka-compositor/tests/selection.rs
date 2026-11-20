use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rikka_clipboard::{Change, Clipboard, Selection};
use rikka_compositor::Extent;
use rikka_test_display::{dispatch_next, dispatch_until, wait_readable};
use rikka_wayland::Host;
use rikka_wayland_client::Queue;
use rikka_wayland_client::wl_data_device::{self, WlDataDevice};
use rikka_wayland_client::wl_data_device_manager::WlDataDeviceManager;
use rikka_wayland_client::wl_data_offer::{self, WlDataOffer};
use rikka_wayland_client::wl_data_source::{self, WlDataSource};
use rikka_wayland_client::wl_keyboard::{self, WlKeyboard};
use x11rb::connection::Connection as _;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SELECTION_NOTIFY_EVENT,
    SelectionNotifyEvent, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::common::client::Client;
use crate::common::shell::Devices;
use crate::common::{Harness, PATIENCE, mapped};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const TEXT: &str = "text/plain;charset=utf-8";
const GREEN: [u8; 4] = [0, 255, 0, 255];
const RED: [u8; 4] = [0, 0, 255, 255];

#[derive(Debug, Default)]
struct Clip {
    entered: Option<u32>,
    offers: Vec<(WlDataOffer, Vec<String>)>,
    selection: Option<WlDataOffer>,
    giving: &'static str,
    given: Vec<String>,
}

impl Clip {
    fn offered(&self) -> Option<&[String]> {
        let selection = self.selection.as_ref()?;
        self.offers
            .iter()
            .find(|(offer, _)| offer == selection)
            .map(|(_, mime_types)| mime_types.as_slice())
    }
}

struct Board {
    queue: Queue<Clip>,
    clip: Clip,
    manager: WlDataDeviceManager,
    device: WlDataDevice,
}

impl Board {
    fn new(client: &Client, devices: &Devices) -> Self {
        let mut queue = client.connection.queue();
        queue.on::<WlKeyboard>(on_wl_keyboard);
        queue.on::<WlDataSource>(on_wl_data_source);
        queue.on::<WlDataDevice>(on_wl_data_device);
        queue.on::<WlDataOffer>(on_wl_data_offer);
        let manager = client.global::<WlDataDeviceManager>(3).via(&queue.route());
        let device = manager.get_data_device(&devices.seat);
        let _keyboard = devices.seat.via(&queue.route()).get_keyboard();
        Self {
            queue,
            clip: Clip::default(),
            manager,
            device,
        }
    }

    fn roundtrip(&mut self) {
        self.queue.roundtrip(&mut self.clip).unwrap();
    }

    fn until(&mut self, what: &str, done: impl Fn(&Clip) -> bool) {
        dispatch_until(&mut self.queue, &mut self.clip, what, PATIENCE, done);
    }

    fn serve_until_closed(&mut self, stop: &std::io::PipeReader) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if dispatch_next(&mut self.queue, &mut self.clip, &[stop.as_fd()], deadline)[0] {
                return;
            }
            assert!(Instant::now() < deadline, "timed out serving the selection");
        }
    }

    fn give(&mut self, text: &'static str) -> WlDataSource {
        self.until("the keyboard", |clip| clip.entered.is_some());
        self.clip.giving = text;
        let source = self.manager.create_data_source();
        source.offer(TEXT);
        self.device
            .set_selection(Some(&source), self.clip.entered.unwrap());
        self.roundtrip();
        source
    }

    fn take(&mut self) -> std::io::PipeReader {
        self.until("a text selection", |clip| {
            clip.offered()
                .is_some_and(|mime_types| mime_types.iter().any(|mime| mime == TEXT))
        });
        let (reader, writer) = std::io::pipe().unwrap();
        let writer = OwnedFd::from(writer);
        self.clip
            .selection
            .as_ref()
            .unwrap()
            .receive(TEXT, writer.as_fd());
        drop(writer);
        self.roundtrip();
        reader
    }
}

fn read_selection(reader: &mut std::io::PipeReader, mut source: Option<&mut Board>) -> String {
    let deadline = Instant::now() + PATIENCE;
    let fd = reader.as_fd().try_clone_to_owned().unwrap();
    rustix::fs::fcntl_setfl(&fd, rustix::fs::OFlags::NONBLOCK).unwrap();
    let mut read = Vec::new();
    let mut chunk = [0; 256];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return String::from_utf8(read).unwrap(),
            Ok(count) => {
                read.extend_from_slice(&chunk[..count]);
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("cannot read selection: {error}"),
        }
        assert!(Instant::now() < deadline, "timed out reading selection");
        match source.as_deref_mut() {
            Some(board) => {
                dispatch_next(&mut board.queue, &mut board.clip, &[fd.as_fd()], deadline);
            }
            None => {
                wait_readable([fd.as_fd()], deadline);
            }
        }
    }
}

#[test]
fn wayland_selection_pastes_across_clients() {
    let harness = Harness::headless(EXTENT);
    let mut giver = harness.client();
    let giver_devices = giver.devices();
    let mut giving = Board::new(&giver, &giver_devices);
    let _giver_window = mapped(&mut giver, 1, (40, 30), GREEN);
    let _source = giving.give("copied across clients");

    let mut taker = harness.client();
    let taker_devices = taker.devices();
    let mut taking = Board::new(&taker, &taker_devices);
    let _taker_window = mapped(&mut taker, 2, (40, 30), RED);
    let mut reader = taking.take();
    let pasted = read_selection(&mut reader, Some(&mut giving));
    assert_eq!(pasted, "copied across clients");
    assert_eq!(giving.clip.given, [TEXT]);
}

#[test]
fn clipboard_sees_own_selection_immediately() {
    let harness = Harness::headless(EXTENT);
    let mut giver = harness.client();
    let giver_devices = giver.devices();
    let mut giving = Board::new(&giver, &giver_devices);
    let _giver_window = mapped(&mut giver, 1, (40, 30), GREEN);
    let _source = giving.give("copied by another client");

    let owner = harness.client();
    let host = Host::with_reader(owner.display()).unwrap();
    let (stop_reader, stop_writer) = std::io::pipe().unwrap();
    let (clipboard, pasted) = std::thread::scope(|scope| {
        scope.spawn(|| giving.serve_until_closed(&stop_reader));
        let clipboard = Clipboard::for_host(&host).unwrap();
        let pasted = clipboard.text(Selection::Clipboard);
        drop(stop_writer);
        (clipboard, pasted)
    });
    assert_eq!(pasted.unwrap(), "copied by another client");

    let (changed, changes) = mpsc::channel::<Change>();
    clipboard
        .watch(move |change| drop(changed.send(change.clone())))
        .unwrap();
    clipboard.set_text("copied beside its owner").unwrap();
    let first = changes
        .recv_timeout(PATIENCE)
        .expect("clipboard reports its own selection");
    assert!(
        first.selection() == Selection::Clipboard && first.is_ours() && first.offers(TEXT),
        "watcher reports only selections made after watching began, got {first:?}"
    );
    assert_eq!(
        clipboard.text(Selection::Clipboard).unwrap(),
        "copied beside its owner"
    );
}

#[test]
fn clipboard_pastes_own_long_text() {
    let harness = Harness::headless(EXTENT);
    let mut owner = harness.client();
    let host = Host::read_by_owner(owner.display());
    let clipboard = Clipboard::for_host(&host).unwrap();
    let (changed, changes) = mpsc::channel::<Change>();
    let (wake_reader, wake_writer) = std::io::pipe().unwrap();
    clipboard
        .watch(move |change| {
            drop(changed.send(change.clone()));
            drop((&wake_writer).write_all(&[1]));
        })
        .unwrap();
    let long = "長い文章を貼り付ける。".repeat(20_000);
    clipboard.set_text(&long).unwrap();
    let deadline = Instant::now() + PATIENCE;
    let change = loop {
        if let Ok(change) = changes.try_recv() {
            break change;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for clipboard to report its own selection"
        );
        let wayland = &mut *owner;
        dispatch_next(
            &mut wayland.queue,
            &mut wayland.received,
            &[wake_reader.as_fd()],
            deadline,
        );
    };
    assert!(change.is_ours() && change.offers(TEXT), "got {change:?}");

    let started = Instant::now();
    let pasted = clipboard.text(Selection::Clipboard);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "pasting own text blocked for {:?} without a reader",
        started.elapsed()
    );
    assert_eq!(pasted.unwrap(), long);
}

fn atom(x11: &RustConnection, name: &str) -> u32 {
    x11.intern_atom(false, name.as_bytes())
        .unwrap()
        .reply()
        .unwrap()
        .atom
}

#[test]
fn x11_selection_pastes_in_wayland() {
    let harness = Harness::headless(EXTENT);
    let (x11, screen) = harness.x11();
    let root = x11.setup().roots[screen].root;
    let owner = x11.generate_id().unwrap();
    x11.create_window(
        0,
        owner,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new(),
    )
    .unwrap();
    let clipboard = atom(&x11, "CLIPBOARD");
    let targets = atom(&x11, "TARGETS");
    let utf8 = atom(&x11, "UTF8_STRING");
    x11.set_selection_owner(owner, clipboard, x11rb::CURRENT_TIME)
        .unwrap();
    x11.flush().unwrap();
    std::thread::spawn(move || {
        while let Ok(event) = x11.wait_for_event() {
            let Event::SelectionRequest(request) = event else {
                continue;
            };
            let answered = if request.target == targets {
                x11.change_property32(
                    PropMode::REPLACE,
                    request.requestor,
                    request.property,
                    AtomEnum::ATOM,
                    &[targets, utf8],
                )
                .map(|_| request.property)
            } else if request.target == utf8 {
                x11.change_property8(
                    PropMode::REPLACE,
                    request.requestor,
                    request.property,
                    utf8,
                    b"owned by x11",
                )
                .map(|_| request.property)
            } else {
                Ok(x11rb::NONE)
            };
            let notify = SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: request.time,
                requestor: request.requestor,
                selection: request.selection,
                target: request.target,
                property: answered.unwrap_or(x11rb::NONE),
            };
            if x11
                .send_event(false, request.requestor, EventMask::NO_EVENT, notify)
                .and_then(|_| x11.flush())
                .is_err()
            {
                break;
            }
        }
    });

    let mut client = harness.client();
    let devices = client.devices();
    let mut board = Board::new(&client, &devices);
    let _window = mapped(&mut client, 1, (40, 30), GREEN);
    let mut reader = board.take();
    let pasted = read_selection(&mut reader, None);
    assert_eq!(pasted, "owned by x11");
}

fn on_wl_keyboard(clip: &mut Clip, _: WlKeyboard, event: wl_keyboard::Event) {
    match event {
        wl_keyboard::Event::Enter { serial, .. } => clip.entered = Some(serial),
        wl_keyboard::Event::Leave { .. } => clip.entered = None,
        _ => {}
    }
}

fn on_wl_data_source(clip: &mut Clip, _: WlDataSource, event: wl_data_source::Event) {
    if let wl_data_source::Event::Send { mime_type, fd } = event {
        let mut pipe = std::fs::File::from(fd);
        pipe.write_all(clip.giving.as_bytes()).unwrap();
        clip.given.push(mime_type);
    }
}

fn on_wl_data_device(clip: &mut Clip, _: WlDataDevice, event: wl_data_device::Event) {
    match event {
        wl_data_device::Event::DataOffer { id } => clip.offers.push((id, Vec::new())),
        wl_data_device::Event::Selection { id } => clip.selection = id,
        _ => {}
    }
}

fn on_wl_data_offer(clip: &mut Clip, offer: WlDataOffer, event: wl_data_offer::Event) {
    if let wl_data_offer::Event::Offer { mime_type } = event
        && let Some((_, mime_types)) = clip.offers.iter_mut().find(|(known, _)| *known == offer)
    {
        mime_types.push(mime_type);
    }
}
