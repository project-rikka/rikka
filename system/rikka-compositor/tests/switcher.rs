use rikka_compositor::{Extent, Injected};
use rikka_seam::Request;
use rikka_session::switcher::{self, Switch, SwitcherClient};

use crate::common::client::Client;
use crate::common::input::{PRESSED, RELEASED};
use crate::common::shell::Devices;
use crate::common::shortcuts::Announced;
use crate::common::{Harness, Toplevel, mapped};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];

const ESCAPE: u32 = 1;
const TAB: u32 = 15;
const ALT: u32 = 56;
const ALT_MASK: u32 = 1 << 3;

struct Desk {
    harness: Harness,
    client: Client,
    windows: Vec<Toplevel>,
    devices: Devices,
}

impl Desk {
    fn of_three() -> Desk {
        let harness = Harness::full(EXTENT, &[]);
        let mut client = harness.client();
        let devices = client.devices();
        let windows: Vec<Toplevel> = (1..=3)
            .map(|tag| mapped(&mut client, tag, (60, 60), GREEN))
            .collect();
        let newest = windows[2].surface.id();
        client.until("keyboard focus on newest window", |recorded| {
            recorded.shell.keyboard_on == Some(newest)
        });
        Desk {
            harness,
            client,
            windows,
            devices,
        }
    }

    fn keys(&self, keys: &[(u32, bool)]) {
        let injected: Vec<Injected> = keys
            .iter()
            .map(|(code, pressed)| Injected::Key {
                code: *code,
                pressed: *pressed,
            })
            .collect();
        self.harness.inject(&injected);
    }

    fn keyboard_on(&mut self, window: usize, what: &str) {
        let surface = self.windows[window].surface.id();
        self.client
            .until(what, |recorded| recorded.shell.keyboard_on == Some(surface));
    }
}

fn listening() -> Announced {
    let announced = Announced::listen();
    let shown: Option<Switch> = announced
        .client()
        .call(Request::new(switcher::CAPABILITY, switcher::STATE))
        .blocking_wait()
        .and_then(|payload| payload.decode())
        .expect("switcher state");
    assert_eq!(shown, None, "no switcher before Alt+Tab");
    announced
}

fn switch(announced: &mut Announced, what: &str) -> Option<Switch> {
    announced.heard(switcher::CAPABILITY, switcher::CHANGED, what)
}

#[test]
fn alt_tab_cycles_in_mru_order() {
    let mut desk = Desk::of_three();

    desk.harness.press(&[ALT, TAB]);
    desk.keyboard_on(1, "Alt+Tab tap switches to previous window");

    desk.harness.press(&[ALT, TAB]);
    desk.keyboard_on(2, "second Alt+Tab tap switches back");

    desk.keys(&[
        (ALT, true),
        (TAB, true),
        (TAB, false),
        (TAB, true),
        (TAB, false),
        (ALT, false),
    ]);
    desk.keyboard_on(0, "two Tabs under one Alt go two windows back");
    assert!(
        !desk.client.received.shell.keys.contains(&TAB),
        "switching Tabs not delivered to windows"
    );
}

#[test]
fn held_switcher_keeps_keyboard_until_choice() {
    let mut desk = Desk::of_three();
    let mut listening = listening();

    desk.keys(&[(ALT, true), (TAB, true), (TAB, false)]);
    desk.client
        .until("keyboard withheld while Alt is held", |recorded| {
            recorded.shell.keyboard_on.is_none()
        });
    let shown =
        switch(&mut listening, "switcher opened").expect("switcher shown while Alt is held");
    assert_eq!(shown.windows.len(), 3);
    assert_eq!(shown.selected, 1);

    desk.keys(&[(ESCAPE, true), (ESCAPE, false)]);
    assert_eq!(switch(&mut listening, "Escape closes switcher"), None);
    desk.keys(&[(ALT, false)]);
    desk.keyboard_on(2, "Escape keeps front window focused");

    desk.keys(&[(ALT, true), (TAB, true), (TAB, false)]);
    let shown = switch(&mut listening, "switcher reopened").expect("switcher shown again");
    SwitcherClient::new(listening.client()).choose(shown.serial, shown.windows[2].id.clone());
    assert_eq!(switch(&mut listening, "choice closes switcher"), None);
    desk.keys(&[(ALT, false)]);
    desk.keyboard_on(0, "window chosen by shell");

    let keys = &desk.client.received.shell.keys;
    assert!(
        !keys.contains(&TAB) && !keys.contains(&ESCAPE),
        "switcher keys not delivered to windows"
    );
}

#[test]
fn shortcut_inhibiting_window_receives_alt_tab() {
    let mut desk = Desk::of_three();
    let seat = desk.devices.seat.clone();
    let _inhibitor = desk
        .client
        .inhibit_shortcuts(&desk.windows[2].surface, &seat);

    desk.keys(&[(ALT, true), (TAB, true), (TAB, false)]);
    desk.client
        .until("Tab delivered to the inhibiting window", |recorded| {
            recorded.shell.keys.contains(&TAB)
        });
    desk.keys(&[(ALT, false)]);
    desk.keys(&[(TAB, true), (TAB, false)]);
    desk.client.until("later keys still delivered", |recorded| {
        recorded
            .shell
            .keys
            .iter()
            .filter(|key| **key == TAB)
            .count()
            == 2
    });
    assert_eq!(
        desk.client.received.shell.keyboard_on,
        Some(desk.windows[2].surface.id()),
        "inhibiting window keeps the keyboard"
    );
}

#[test]
fn virtual_keyboard_releasing_alt_ends_the_switch() {
    let mut desk = Desk::of_three();
    let seat = desk.devices.seat.clone();
    let keyboard = desk.client.virtual_keyboard(&seat);

    keyboard.modifiers(ALT_MASK, 0, 0, 0);
    keyboard.send_key(0, TAB, PRESSED);
    keyboard.send_key(0, TAB, RELEASED);
    desk.client.until(
        "keyboard withheld while the virtual Alt is held",
        |recorded| recorded.shell.keyboard_on.is_none(),
    );
    keyboard.modifiers(0, 0, 0, 0);
    desk.keyboard_on(1, "clearing the virtual Alt chooses the previous window");

    keyboard.send_key(0, ALT, PRESSED);
    keyboard.send_key(0, TAB, PRESSED);
    keyboard.send_key(0, TAB, RELEASED);
    desk.client.until(
        "keyboard withheld while the virtual Alt key is down",
        |recorded| recorded.shell.keyboard_on.is_none(),
    );
    keyboard.destroy();
    desk.keyboard_on(2, "a vanished virtual keyboard lets go of its Alt");
}
