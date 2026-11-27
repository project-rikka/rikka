use std::io::Read;
use std::os::fd::AsFd;
use std::sync::mpsc::{self, Receiver};

use rikka_clipboard::{Clipboard, Offer, Selection};
use rikka_nested::{Claim, Input, Nested, View};
use rikka_wayland_client::wl_data_device::WlDataDevice;
use rustix::pipe::{PipeFlags, pipe_with};

use crate::support::session::Session;
use crate::support::{self, Client, TIMEOUT, roundtrip_until};

const SESSION_ONLY: &str = "UTF8_STRING";

fn bridged(session: &Session) -> (Nested, Clipboard) {
    let clipboard = session.clipboard();
    let nested = Nested::builder()
        .gpu(support::gpu())
        .clipboard(clipboard.share())
        .start()
        .expect("the nested display starts");
    (nested, clipboard)
}

fn focused(nested: &Nested, app_id: &str) -> (View, Client, WlDataDevice) {
    let view = nested.view(Claim::app_id(app_id));
    let mut client = Client::connect(nested.socket());
    client.follow(&view);
    let _keyboard = client.keyboard();
    let device = client.data_device();
    let window = client.window(Some(app_id));
    client.map(&window, 40, 40);
    roundtrip_until(&mut client, |_| view.look().attached);
    view.deliver(Input::Focus(true));
    roundtrip_until(&mut client, |client| {
        client
            .received
            .keyboard
            .iter()
            .any(|line| line.starts_with("enter"))
    });
    (view, client, device)
}

fn announcements(
    session: &Session,
    notify: impl Fn() + Send + 'static,
) -> (Clipboard, Receiver<Vec<String>>) {
    let watcher = session.clipboard();
    let (sender, announced) = mpsc::channel();
    watcher
        .watch(move |change| {
            let _ = sender.send(change.mime_types().to_vec());
            notify();
        })
        .unwrap();
    watcher
        .mime_types(Selection::Clipboard)
        .expect("the watch is registered");
    (watcher, announced)
}

fn offers(mimes: &[String], mime: &str) -> bool {
    mimes.iter().any(|offered| offered == mime)
}

fn published(session: &Session, text: &str) -> Clipboard {
    let (_watcher, announced) = announcements(session, || {});
    let publisher = session.clipboard();
    publisher
        .publish(Selection::Clipboard, Offer::text(text))
        .unwrap();
    loop {
        let mimes = announced
            .recv_timeout(TIMEOUT)
            .expect("the session announces the selection");
        if offers(&mimes, SESSION_ONLY) {
            return publisher;
        }
    }
}

fn pasted(client: &mut Client, mime: &str) -> String {
    roundtrip_until(client, |client| {
        client
            .received
            .selection
            .and_then(|offer| client.received.offers.get(&offer))
            .is_some_and(|mimes| offers(mimes, mime))
    });
    let offer = client.received.data_offers[&client.received.selection.unwrap()].clone();
    let (read, write) = pipe_with(PipeFlags::CLOEXEC).unwrap();
    offer.receive(mime, write.as_fd());
    drop(write);
    client.roundtrip();
    let mut text = String::new();
    std::fs::File::from(read).read_to_string(&mut text).unwrap();
    text
}

#[test]
fn session_clipboard_reaches_focused_client() {
    for published_first in [true, false] {
        let session = Session::start();
        let early = published_first.then(|| published(&session, "from the session"));
        let (nested, _bridge) = bridged(&session);
        let (_view, mut client, _device) = focused(&nested, "rikka-view-paste");
        let late = (!published_first).then(|| published(&session, "from the session"));
        assert_eq!(
            pasted(&mut client, SESSION_ONLY),
            "from the session",
            "published first: {published_first}"
        );
        drop((early, late));
    }
}

#[test]
fn client_copy_reaches_session_clipboard() {
    for (refocused, read_by_bridge) in [(false, false), (true, false), (false, true)] {
        let session = Session::start();
        let (nested, bridge) = bridged(&session);
        let (view, mut client, device) = focused(&nested, "rikka-view-copy");
        let serial = client.received.entered;
        if refocused {
            view.deliver(Input::Focus(false));
            view.deliver(Input::Focus(true));
            roundtrip_until(&mut client, |client| client.received.entered != serial);
        }
        let (watcher, announced) = announcements(&session, client.notifier());
        let source = client.source(&["text/plain;charset=utf-8"]);
        device.set_selection(Some(&source), serial);
        source.offer("SAVE_TARGETS");
        device.set_selection(Some(&source), serial);
        roundtrip_until(&mut client, |_| {
            announced
                .try_iter()
                .any(|mimes| offers(&mimes, "SAVE_TARGETS"))
        });
        let reader = match read_by_bridge {
            true => bridge.share(),
            false => watcher,
        };
        let notify = client.notifier();
        let reading = std::thread::spawn(move || {
            let text = reader.text(Selection::Clipboard);
            notify();
            text
        });
        roundtrip_until(&mut client, |_| reading.is_finished());
        assert_eq!(
            reading.join().unwrap().ok().as_deref(),
            Some("nested text/plain;charset=utf-8"),
            "refocused: {refocused}, read by the bridge: {read_by_bridge}"
        );
    }
}

#[test]
fn client_clearing_its_copy_clears_session_clipboard() {
    let session = Session::start();
    let (nested, _bridge) = bridged(&session);
    let (_view, mut client, device) = focused(&nested, "rikka-view-clear");
    let (_watcher, announced) = announcements(&session, client.notifier());
    let source = client.source(&["text/plain;charset=utf-8"]);
    device.set_selection(Some(&source), client.received.entered);
    roundtrip_until(&mut client, |_| {
        announced
            .try_iter()
            .any(|mimes| mimes == ["text/plain;charset=utf-8"])
    });
    device.set_selection(None, client.received.entered);
    roundtrip_until(&mut client, |_| {
        announced.try_iter().any(|mimes| mimes.is_empty())
    });
}

#[test]
fn session_copy_after_client_copy_reaches_client() {
    let session = Session::start();
    let (nested, _bridge) = bridged(&session);
    let (_view, mut client, device) = focused(&nested, "rikka-view-recopy");
    let (_watcher, announced) = announcements(&session, client.notifier());
    let source = client.source(&["text/plain;charset=utf-8"]);
    device.set_selection(Some(&source), client.received.entered);
    roundtrip_until(&mut client, |_| {
        announced
            .try_iter()
            .any(|mimes| mimes == ["text/plain;charset=utf-8"])
    });
    let _publisher = published(&session, "copied elsewhere");
    assert_eq!(pasted(&mut client, SESSION_ONLY), "copied elsewhere");
}
