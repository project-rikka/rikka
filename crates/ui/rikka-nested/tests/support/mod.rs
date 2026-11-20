pub(crate) mod client;
pub(crate) mod session;

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use rikka_input::xkb::{KeyRepeat, SessionKeyboard};
use rikka_nested::{Event, Nested};
use rikka_picture::SharedGpu;

pub(crate) use client::{Client, Window};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) const KEYMAP: &str = r#"xkb_keymap {
    xkb_keycodes "rikka" {
        minimum = 8;
        maximum = 255;
        <ESC> = 9;
        <AC01> = 38;
        <LFSH> = 50;
    };
    xkb_types "rikka" {
        type "ONE_LEVEL" {
            modifiers = none;
            level_name[Level1] = "Any";
        };
    };
    xkb_compatibility "rikka" {
        interpret Shift_L { action = SetMods(modifiers = Shift); };
    };
    xkb_symbols "rikka" {
        key <ESC> { [ Escape ] };
        key <AC01> { [ a ] };
        key <LFSH> { [ Shift_L ] };
        modifier_map Shift { <LFSH> };
    };
};
"#;

pub(crate) fn gpu() -> SharedGpu {
    let format = |code: &[u8; 4]| rikka_gpu::DrmFormat {
        fourcc: u32::from_le_bytes(*code),
        modifier: 0,
    };
    SharedGpu {
        primary_node: None,
        render_node: Some((226, 128)),
        texture_formats: vec![format(b"XR24"), format(b"AR24")],
    }
}

pub(crate) fn start() -> (Nested, Receiver<Event>) {
    SessionKeyboard::publish_keymap(KEYMAP);
    SessionKeyboard::publish_repeat(KeyRepeat {
        rate: 33,
        delay: 400,
    });
    let nested = Nested::builder()
        .gpu(gpu())
        .start()
        .expect("the nested display starts");
    let events = nested.events().expect("the events are untaken");
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for event in events {
            if sender.send(event).is_err() {
                return;
            }
        }
    });
    (nested, receiver)
}

pub(crate) fn await_event(events: &Receiver<Event>, expected: &Event) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = events
            .recv_timeout(left)
            .unwrap_or_else(|_| panic!("never received {expected:?}"));
        if event == *expected {
            return;
        }
    }
}

pub(crate) fn roundtrip_until(client: &mut Client, mut done: impl FnMut(&mut Client) -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        client.roundtrip();
        if done(client) {
            return;
        }
        assert!(client.wait(deadline), "the nested display never settled");
    }
}
