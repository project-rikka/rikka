use rikka_compositor::{Extent, Injected};
use rikka_theme::Theme;
use rikka_wayland_client::zxdg_toplevel_decoration_v1::Mode;

use crate::common::{BTN_LEFT, Client, Harness, Toplevel, shows};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];
const SHOWN_GREEN: [u8; 4] = [0, 255, 0, 255];

const MAXIMIZED: u32 = 1;

const CAPTION: i32 = 39;

const CLOSE: (f64, f64) = (225.0, 67.0);

const TITLE: (f64, f64) = (100.0, 60.0);

fn framed(client: &mut Client, tag: u32, mode: Mode) -> Toplevel {
    let window = client.decorated_toplevel(tag, mode);
    client.ack(&window.xdg, tag);
    let pixels = client.pixels(200, 80, tag);
    pixels.fill(0..80, GREEN);
    client.show(&window.surface, &pixels);
    window
}

fn caption_fill() -> [u8; 4] {
    Theme::default().palette().background.to_rgba8()
}

#[test]
fn server_side_window_wears_a_compositor_caption() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = framed(&mut client, 1, Mode::ServerSide);

    assert_eq!(client.received.shell.decorations, [(1, Mode::ServerSide)]);
    let fill = caption_fill();
    harness.frame_where(move |snapshot| {
        shows(snapshot, 100, 50, fill)
            && shows(snapshot, 100, 48 + CAPTION as u32 + 2, SHOWN_GREEN)
            && shows(snapshot, 240, 160, SHOWN_GREEN)
            && !shows(snapshot, 100, 50, SHOWN_GREEN)
    });
}

#[test]
fn client_side_window_draws_itself() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = framed(&mut client, 1, Mode::ClientSide);

    assert_eq!(client.received.shell.decorations, [(1, Mode::ClientSide)]);
    harness.frame_where(move |snapshot| {
        shows(snapshot, 100, 50, SHOWN_GREEN) && !shows(snapshot, 100, 130, SHOWN_GREEN)
    });
}

#[test]
fn caption_close_button_asks_to_close() {
    let cases: [(&str, Vec<Injected>); 2] = [
        (
            "pointer",
            vec![
                Injected::PointerTo {
                    x: CLOSE.0,
                    y: CLOSE.1,
                },
                Injected::Button {
                    code: BTN_LEFT,
                    pressed: true,
                },
                Injected::Button {
                    code: BTN_LEFT,
                    pressed: false,
                },
            ],
        ),
        (
            "touch",
            vec![
                Injected::TouchDown {
                    slot: 0,
                    x: CLOSE.0,
                    y: CLOSE.1,
                },
                Injected::TouchFrame,
                Injected::TouchUp { slot: 0 },
                Injected::TouchFrame,
            ],
        ),
    ];
    for (device, steps) in cases {
        let harness = Harness::headless(EXTENT);
        let mut client = harness.client();
        let _devices = client.devices();
        let _window = framed(&mut client, 1, Mode::ServerSide);
        harness.frame_where(move |snapshot| shows(snapshot, 240, 160, SHOWN_GREEN));

        harness.inject(&steps);
        client.until(&format!("{device} close asked"), |recorded| {
            recorded.shell.closed.contains(&1)
        });
    }
}

#[test]
fn caption_press_leaving_the_button_does_nothing() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = framed(&mut client, 1, Mode::ServerSide);
    harness.frame_where(move |snapshot| shows(snapshot, 240, 160, SHOWN_GREEN));

    harness.inject(&[
        Injected::PointerTo {
            x: CLOSE.0,
            y: CLOSE.1,
        },
        Injected::Button {
            code: BTN_LEFT,
            pressed: true,
        },
        Injected::PointerTo { x: 225.0, y: 150.0 },
        Injected::Button {
            code: BTN_LEFT,
            pressed: false,
        },
    ]);
    harness.click(CLOSE.0, CLOSE.1);
    client.until("close asked by the full click", |recorded| {
        !recorded.shell.closed.is_empty()
    });
    client.roundtrip();
    assert_eq!(client.received.shell.closed, [1]);
}

#[test]
fn double_click_on_title_maximizes_below_the_caption() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = framed(&mut client, 1, Mode::ServerSide);
    harness.frame_where(move |snapshot| shows(snapshot, 240, 160, SHOWN_GREEN));

    harness.click(TITLE.0, TITLE.1);
    harness.click(TITLE.0, TITLE.1);
    client.until("window maximized under its caption", |recorded| {
        recorded.shell.last_configure(1).is_some_and(|configure| {
            configure.states.contains(&MAXIMIZED)
                && (configure.width, configure.height) == (320, 240 - CAPTION)
        })
    });
}

#[test]
fn title_drag_moves_the_window() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = framed(&mut client, 1, Mode::ServerSide);
    harness.frame_where(move |snapshot| shows(snapshot, 60, 100, SHOWN_GREEN));

    harness.inject(&[
        Injected::PointerTo {
            x: TITLE.0,
            y: TITLE.1,
        },
        Injected::Button {
            code: BTN_LEFT,
            pressed: true,
        },
        Injected::PointerTo {
            x: TITLE.0 + 10.0,
            y: TITLE.1,
        },
        Injected::PointerTo {
            x: TITLE.0 + 60.0,
            y: TITLE.1 + 60.0,
        },
        Injected::Button {
            code: BTN_LEFT,
            pressed: false,
        },
    ]);
    harness.frame_where(move |snapshot| {
        shows(snapshot, 150, 200, SHOWN_GREEN) && !shows(snapshot, 60, 100, SHOWN_GREEN)
    });
}

#[test]
fn outer_edge_resizes_the_content() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = framed(&mut client, 1, Mode::ServerSide);
    harness.frame_where(move |snapshot| shows(snapshot, 240, 160, SHOWN_GREEN));

    harness.inject(&[
        Injected::PointerTo { x: 44.0, y: 120.0 },
        Injected::Button {
            code: BTN_LEFT,
            pressed: true,
        },
        Injected::PointerTo { x: 24.0, y: 120.0 },
        Injected::Button {
            code: BTN_LEFT,
            pressed: false,
        },
    ]);
    client.until("content widened by the edge drag", |recorded| {
        recorded
            .shell
            .last_configure(1)
            .is_some_and(|configure| (configure.width, configure.height) == (220, 80))
    });
}
