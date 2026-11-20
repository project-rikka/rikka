use rikka_compositor::Extent;
use rikka_seam::Request;
use rikka_session::lock::{self, LockState};
use rikka_session::toplevels::{self, ToplevelRef};
use rikka_settings::seam::protocol;
use rikka_settings::session::windows;
use rikka_theme::Theme;
use rikka_wayland_client::zwlr_layer_surface_v1::Anchor;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, WindowClass};

use crate::common::shortcuts::Announced;
use crate::common::{Harness, mapped, shows, tile};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const TILING: [(&str, &str); 1] = [("rikka.windows", "layout = \"tiling\"\n")];
const SCROLLING: [(&str, &str); 1] = [("rikka.windows", "layout = \"tiling\"\nscrolling = true\n")];

const KEY_LEFTMETA: u32 = 125;
const KEY_LEFT: u32 = 105;
const KEY_RIGHTBRACE: u32 = 27;

const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const RED: [u8; 4] = [0, 0, 255, 255];
const SHOWN_GREEN: [u8; 4] = [0, 255, 0, 255];
const SHOWN_BLUE: [u8; 4] = [0, 0, 255, 255];
const SHOWN_RED: [u8; 4] = [255, 0, 0, 255];
const X11_RED: u32 = 0x00ff_0000;

const MAXIMIZED: u32 = 1;
const ACTIVATED: u32 = 4;
const TILED_LEFT: u32 = 5;

#[test]
fn toplevel_is_configured_mapped_and_focused() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let window = mapped(&mut client, 1, (40, 30), GREEN);

    let first = client.received.shell.configured[0].clone();
    assert_eq!(
        (first.width, first.height),
        (0, 0),
        "floating window chooses its size"
    );
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));
    let id = window.surface.id();
    client.until("keyboard focus on new window", |recorded| {
        recorded.shell.keyboard_on == Some(id)
    });
    client.until("window configured as activated", |recorded| {
        recorded
            .shell
            .last_configure(1)
            .is_some_and(|configure| configure.states.contains(&ACTIVATED))
    });
}

#[test]
fn click_focuses_window() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let below = mapped(&mut client, 1, (60, 60), GREEN);
    let above = mapped(&mut client, 2, (60, 60), BLUE);
    let (below, above) = (below.surface.id(), above.surface.id());
    client.until("keyboard focus on newest window", |recorded| {
        recorded.shell.keyboard_on == Some(above)
    });

    harness.click(52.0, 52.0);
    client.until("keyboard focus follows click", |recorded| {
        recorded.shell.keyboard_on == Some(below) && recorded.shell.pointer_on == Some(below)
    });
    harness.frame_where(move |snapshot| shows(snapshot, 100, 100, SHOWN_GREEN));
}

#[test]
fn grabbing_popup_dismissed_by_outside_click() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let devices = client.devices();
    let window = mapped(&mut client, 1, (100, 80), GREEN);

    harness.click(60.0, 60.0);
    client.until("click delivered", |recorded| {
        !recorded.shell.pressed.is_empty()
    });
    let serial = *client.received.shell.pressed.last().unwrap();
    let popup = client.popup(&window, 7, (10, 10), (30, 20));
    popup.popup.grab(&devices.seat, serial);
    popup.surface.commit();
    client.until("popup configured", |recorded| {
        recorded.shell.serial(7).is_some() && !recorded.shell.popup_placed.is_empty()
    });
    client.ack(&popup.xdg, 7);
    let pixels = client.pixels(30, 20, 70);
    pixels.fill(0..20, BLUE);
    client.show(&popup.surface, &pixels);
    harness.frame_where(move |snapshot| shows(snapshot, 80, 72, SHOWN_BLUE));

    harness.click(300.0, 220.0);
    client.until("popup dismissed", |recorded| {
        recorded.shell.popup_done.contains(&7)
    });
}

#[test]
fn exclusive_zone_shrinks_maximized_window() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let (bar, _layer) = client.layer(3, Anchor::TOP | Anchor::LEFT | Anchor::RIGHT, (0, 20), 20);
    let (_, _, width, height) = client.received.shell.layer_configured[0];
    assert_eq!((width, height), (320, 20));
    let pixels = client.pixels(320, 20, 30);
    pixels.fill(0..20, BLUE);
    client.show(&bar, &pixels);

    let window = mapped(&mut client, 1, (40, 30), GREEN);
    window.toplevel.set_maximized();
    client.until("window configured to fill work area", |recorded| {
        recorded.shell.last_configure(1).is_some_and(|configure| {
            configure.states.contains(&MAXIMIZED)
                && (configure.width, configure.height) == (320, 220)
        })
    });
    harness.frame_where(move |snapshot| shows(snapshot, 200, 10, SHOWN_BLUE));
}

#[test]
fn toplevels_seam_maximizes_and_restores_window() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let _window = mapped(&mut client, 1, (40, 30), GREEN);
    let listed: toplevels::State =
        harness.ask(Request::new(toplevels::CAPABILITY, toplevels::STATE));
    let id = listed.toplevels[0].id.clone();
    assert!(!listed.toplevels[0].maximized);

    let _: () = harness.ask(
        Request::new(toplevels::CAPABILITY, toplevels::MAXIMIZE).encode(&ToplevelRef::new(&id)),
    );
    client.until("window configured maximized", |recorded| {
        recorded
            .shell
            .last_configure(1)
            .is_some_and(|configure| configure.states.contains(&MAXIMIZED))
    });
    let listed: toplevels::State =
        harness.ask(Request::new(toplevels::CAPABILITY, toplevels::STATE));
    assert!(listed.toplevels[0].maximized);

    let _: () = harness.ask(
        Request::new(toplevels::CAPABILITY, toplevels::UNMAXIMIZE).encode(&ToplevelRef::new(&id)),
    );
    client.until("window configured restored", |recorded| {
        recorded
            .shell
            .last_configure(1)
            .is_some_and(|configure| !configure.states.contains(&MAXIMIZED))
    });
    let listed: toplevels::State =
        harness.ask(Request::new(toplevels::CAPABILITY, toplevels::STATE));
    assert!(!listed.toplevels[0].maximized);
}

#[test]
fn modal_dialog_holds_focus_over_its_parent() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let parent = mapped(&mut client, 1, (200, 160), GREEN);
    let dialog = client.dialog(2, &parent, true);
    client.ack(&dialog.xdg, 2);
    let pixels = client.pixels(60, 40, 2);
    pixels.fill(0..40, BLUE);
    client.show(&dialog.surface, &pixels);
    let dialog = dialog.surface.id();
    client.until("modal dialog focused", |recorded| {
        recorded.shell.keyboard_on == Some(dialog)
    });
    harness.frame_where(move |snapshot| {
        shows(snapshot, 124, 128, SHOWN_BLUE) && shows(snapshot, 172, 128, SHOWN_BLUE)
    });

    let before = client.received.shell.pressed.len();
    harness.click(60.0, 60.0);
    harness.click(148.0, 128.0);
    client.until("only the dialog hears presses", |recorded| {
        recorded.shell.pointer_on == Some(dialog) && recorded.shell.pressed.len() == before + 1
    });
    client.roundtrip();
    assert_eq!(client.received.shell.pressed.len(), before + 1);
    assert_eq!(client.received.shell.keyboard_on, Some(dialog));
    harness.frame_where(move |snapshot| shows(snapshot, 148, 128, SHOWN_BLUE));
}

#[test]
fn locked_session_shows_only_lock_surface() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let output = client.output();
    mapped(&mut client, 1, (60, 60), GREEN);
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));

    let lock = client.lock();
    let surface = client.surface();
    let _guard = lock.get_lock_surface(&surface, &output);
    client.until("lock surface configured", |recorded| {
        !recorded.shell.lock_configured.is_empty()
    });
    let (_, width, height) = client.received.shell.lock_configured[0];
    assert_eq!((width, height), (320, 240));
    let pixels = client.pixels(320, 240, 40);
    pixels.fill(0..240, BLUE);
    client.show(&surface, &pixels);
    client.until("session locked", |recorded| recorded.shell.locked);
    let locked = harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_BLUE));
    assert!(shows(&locked, 300, 220, SHOWN_BLUE));
    client.until("keyboard focus on lock surface", |recorded| {
        recorded.shell.keyboard_on == Some(surface.id())
    });

    lock.unlock_and_destroy();
    client.roundtrip();
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));
}

#[test]
fn session_stays_locked_after_locker_exits() {
    let harness = Harness::headless(EXTENT);
    let mut desk = harness.client();
    let output = desk.output();
    let _window = mapped(&mut desk, 1, (60, 60), GREEN);
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));
    assert_eq!(lock_state(&harness), LockState::Open);

    {
        let mut locker = harness.client();
        let output = locker.output();
        let lock = locker.lock();
        let surface = locker.surface();
        let _guard = lock.get_lock_surface(&surface, &output);
        locker.until("lock surface configured", |recorded| {
            !recorded.shell.lock_configured.is_empty()
        });
        let pixels = locker.pixels(320, 240, 50);
        pixels.fill(0..240, BLUE);
        locker.show(&surface, &pixels);
        locker.until("session locked", |recorded| recorded.shell.locked);
        assert_eq!(lock_state(&harness), LockState::Locked);
    }

    until_lock_state(LockState::Abandoned, "locker exit detected");
    let abandoned = harness.frame_where(move |snapshot| !shows(snapshot, 300, 220, SHOWN_BLUE));
    assert!(!shows(&abandoned, 60, 60, SHOWN_GREEN));

    let lock = desk.lock();
    let surface = desk.surface();
    let _guard = lock.get_lock_surface(&surface, &output);
    desk.until("new lock surface configured", |recorded| {
        !recorded.shell.lock_configured.is_empty()
    });
    let pixels = desk.pixels(320, 240, 51);
    pixels.fill(0..240, RED);
    desk.show(&surface, &pixels);
    desk.until("session locked again", |recorded| recorded.shell.locked);
    assert_eq!(lock_state(&harness), LockState::Locked);
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_RED));

    lock.unlock_and_destroy();
    desk.roundtrip();
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));
    assert_eq!(lock_state(&harness), LockState::Open);
}

#[test]
fn locked_start_shows_nothing_until_unlock() {
    let harness = Harness::starting_locked(EXTENT, &[]);
    assert_eq!(lock_state(&harness), LockState::Sealed);
    let mut desk = harness.client();
    let output = desk.output();
    let _window = mapped(&mut desk, 1, (60, 60), GREEN);
    desk.roundtrip();
    let sealed = harness.frame_where(move |_| true);
    assert!(!shows(&sealed, 60, 60, SHOWN_GREEN));

    {
        let mut early = harness.client();
        let output = early.output();
        let lock = early.lock();
        let surface = early.surface();
        let _guard = lock.get_lock_surface(&surface, &output);
        early.until("early lock surface configured", |recorded| {
            !recorded.shell.lock_configured.is_empty()
        });
    }
    until_lock_state(LockState::Abandoned, "early locker exit detected");
    let left = harness.frame_where(move |_| true);
    assert!(!shows(&left, 60, 60, SHOWN_GREEN));

    let lock = desk.lock();
    let surface = desk.surface();
    let _guard = lock.get_lock_surface(&surface, &output);
    desk.until("lock surface configured", |recorded| {
        !recorded.shell.lock_configured.is_empty()
    });
    let pixels = desk.pixels(320, 240, 52);
    pixels.fill(0..240, RED);
    desk.show(&surface, &pixels);
    desk.until("session locked", |recorded| recorded.shell.locked);
    assert_eq!(lock_state(&harness), LockState::Locked);
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_RED));

    lock.unlock_and_destroy();
    desk.roundtrip();
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));
    assert_eq!(lock_state(&harness), LockState::Open);
}

#[test]
fn unlocked_start_skips_lock() {
    let harness = Harness::starting_locked(EXTENT, &[("rikka.lock", "at-login = false\n")]);
    assert_eq!(lock_state(&harness), LockState::Open);
    let mut desk = harness.client();
    let _window = mapped(&mut desk, 1, (60, 60), GREEN);
    harness.frame_where(move |snapshot| shows(snapshot, 60, 60, SHOWN_GREEN));
}

fn lock_state(harness: &Harness) -> LockState {
    harness.ask(lock::PROTOCOL.method(lock::STATE))
}

fn until_lock_state(expected: LockState, what: &str) {
    let (mut announced, state): (Announced, LockState) =
        Announced::subscribe(lock::PROTOCOL.method(lock::STATE));
    if state == expected {
        return;
    }
    announced.until(
        lock::CAPABILITY,
        lock::CHANGED,
        what,
        |state: &LockState| *state == expected,
    );
}

#[test]
fn two_windows_tile_side_by_side() {
    let harness = Harness::full(EXTENT, &TILING);
    let mut client = harness.client();
    let _first = tile(&mut client, 1, GREEN);
    let alone = client.received.shell.last_configure(1).cloned().unwrap();
    assert_eq!((alone.width, alone.height), (304, 224));

    let _second = tile(&mut client, 2, BLUE);
    client.until("first window configured to make room", |recorded| {
        recorded.shell.last_configure(1).is_some_and(|configure| {
            (configure.width, configure.height) == (148, 224)
                && configure.states.contains(&TILED_LEFT)
        })
    });
    let second = client.received.shell.last_configure(2).cloned().unwrap();
    assert_eq!((second.width, second.height), (148, 224));
    harness.frame_where(move |snapshot| shows(snapshot, 200, 100, SHOWN_BLUE));
}

#[test]
fn focused_tile_is_outlined_in_the_selection_colour() {
    let harness = Harness::full(EXTENT, &TILING);
    let mut client = harness.client();
    let first = tile(&mut client, 1, GREEN);
    let _second = tile(&mut client, 2, BLUE);
    client.until("first window configured to make room", |recorded| {
        recorded
            .shell
            .last_configure(1)
            .is_some_and(|configure| (configure.width, configure.height) == (148, 224))
    });
    client.ack(&first.xdg, 1);
    let pixels = client.pixels(148, 224, 1);
    pixels.fill(0..224, GREEN);
    client.show(&first.surface, &pixels);

    let selection = Theme::default().palette().selection.to_rgba8();
    harness.frame_where(move |snapshot| {
        shows(snapshot, 163, 100, selection)
            && shows(snapshot, 312, 100, selection)
            && shows(snapshot, 200, 7, selection)
            && !shows(snapshot, 157, 100, selection)
            && shows(snapshot, 200, 100, SHOWN_BLUE)
    });

    harness.click(60.0, 100.0);
    harness.frame_where(move |snapshot| {
        shows(snapshot, 157, 100, selection)
            && shows(snapshot, 7, 100, selection)
            && !shows(snapshot, 163, 100, selection)
            && shows(snapshot, 120, 60, SHOWN_GREEN)
    });
}

#[test]
fn focus_outline_goes_when_turned_off() {
    let harness = Harness::full(EXTENT, &TILING);
    let mut client = harness.client();
    let _only = tile(&mut client, 1, GREEN);
    let selection = Theme::default().palette().selection.to_rgba8();
    harness.frame_where(move |snapshot| shows(snapshot, 7, 100, selection));

    let _: serde_json::Value = harness.ask(protocol::set(
        windows::DOMAIN,
        windows::FOCUS_OUTLINE,
        false,
    ));
    harness.frame_where(move |snapshot| {
        snapshot.pixel(7, 100) == snapshot.pixel(2, 100)
            && snapshot.pixel(312, 100) == snapshot.pixel(317, 100)
            && shows(snapshot, 100, 100, SHOWN_GREEN)
    });
}

#[test]
fn tiles_have_rounded_corners_and_an_edge() {
    let harness = Harness::full(EXTENT, &TILING);
    let mut client = harness.client();
    let first = tile(&mut client, 1, GREEN);
    let _second = tile(&mut client, 2, BLUE);
    client.until("first window configured to make room", |recorded| {
        recorded
            .shell
            .last_configure(1)
            .is_some_and(|configure| (configure.width, configure.height) == (148, 224))
    });
    client.ack(&first.xdg, 1);
    let pixels = client.pixels(148, 224, 1);
    pixels.fill(0..224, GREEN);
    client.show(&first.surface, &pixels);

    harness.frame_where(move |snapshot| {
        shows(snapshot, 140, 24, SHOWN_GREEN)
            && !shows(snapshot, 154, 9, SHOWN_GREEN)
            && shows(snapshot, 180, 24, SHOWN_BLUE)
            && !shows(snapshot, 165, 9, SHOWN_BLUE)
            && shows(snapshot, 166, 100, SHOWN_BLUE)
            && !shows(snapshot, 164, 100, SHOWN_BLUE)
    });
}

#[test]
fn maximized_window_has_square_corners() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let window = mapped(&mut client, 1, (40, 30), GREEN);
    window.toplevel.set_maximized();
    client.until("window configured to fill the display", |recorded| {
        recorded.shell.last_configure(1).is_some_and(|configure| {
            configure.states.contains(&MAXIMIZED)
                && (configure.width, configure.height) == (320, 240)
        })
    });
    client.ack(&window.xdg, 1);
    let pixels = client.pixels(320, 240, 1);
    pixels.fill(0..240, GREEN);
    client.show(&window.surface, &pixels);
    harness.frame_where(move |snapshot| {
        [(319, 0), (0, 239), (319, 239), (319, 120)]
            .into_iter()
            .all(|(x, y)| shows(snapshot, x, y, SHOWN_GREEN))
    });
}

#[test]
fn columns_open_beside_focus_and_scroll() {
    let harness = Harness::full(EXTENT, &SCROLLING);
    let mut client = harness.client();
    let _first = tile(&mut client, 1, GREEN);
    let _second = tile(&mut client, 2, BLUE);
    let _third = tile(&mut client, 3, RED);
    for tag in 1..=3 {
        let configure = client.received.shell.last_configure(tag).cloned().unwrap();
        assert_eq!(
            (configure.width, configure.height),
            (148, 224),
            "window {tag}"
        );
    }
    harness.frame_where(move |snapshot| {
        shows(snapshot, 60, 100, SHOWN_BLUE) && shows(snapshot, 220, 100, SHOWN_RED)
    });

    harness.press(&[KEY_LEFTMETA, KEY_LEFT]);
    harness.press(&[KEY_LEFTMETA, KEY_LEFT]);
    harness.frame_where(move |snapshot| {
        shows(snapshot, 60, 100, SHOWN_GREEN) && shows(snapshot, 220, 100, SHOWN_BLUE)
    });

    harness.press(&[KEY_LEFTMETA, KEY_RIGHTBRACE]);
    client.until("first window stacked above second", |recorded| {
        [1, 2].into_iter().all(|tag| {
            recorded
                .shell
                .last_configure(tag)
                .is_some_and(|configure| (configure.width, configure.height) == (148, 108))
        })
    });
}

#[test]
fn x11_window_is_mapped_and_drawn() {
    let harness = Harness::headless(EXTENT);
    let (x11, screen) = harness.x11();
    let root = x11.setup().roots[screen].root;
    let depth = x11.setup().roots[screen].root_depth;
    let visual = x11.setup().roots[screen].root_visual;
    let window = x11.generate_id().unwrap();
    x11.create_window(
        depth,
        window,
        root,
        0,
        0,
        60,
        40,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new().background_pixel(X11_RED),
    )
    .unwrap();
    x11.map_window(window).unwrap();
    x11.flush().unwrap();

    let fill = Theme::default().palette().background.to_rgba8();
    harness.frame_where(move |snapshot| {
        shows(snapshot, 70, 100, SHOWN_RED)
            && shows(snapshot, 70, 50, fill)
            && !shows(snapshot, 70, 70, SHOWN_RED)
    });
    let extents = x11
        .intern_atom(false, b"_NET_FRAME_EXTENTS")
        .unwrap()
        .reply()
        .unwrap()
        .atom;
    let published = x11
        .get_property(false, window, extents, AtomEnum::CARDINAL, 0, 4)
        .unwrap()
        .reply()
        .unwrap();
    assert_eq!(
        published.value32().map(Iterator::collect::<Vec<u32>>),
        Some(vec![0, 0, 39, 0])
    );
    let geometry = x11.get_geometry(window).unwrap().reply().unwrap();
    assert_eq!((geometry.width, geometry.height), (60, 40));
}
