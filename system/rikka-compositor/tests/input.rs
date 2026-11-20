use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Wake, Waker};
use std::time::Duration;

use rikka_compositor::{Extent, Injected};
use rikka_ime::{Caret, PanelAnchor};
use rikka_seam::Request;
use rikka_session::agent::{
    AgentPlacement, CAPABILITY, Idle, PLACEMENT, SEAT, SHOW, WAIT_IDLE, WaitIdleRequest,
};
use rikka_session::shortcuts::Shortcut;
use rikka_wayland::{Host, Serial, SerialKind};
use rikka_wayland_client::wl_pointer::ButtonState;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwlr_layer_shell_v1::Layer as Level;
use rikka_wayland_client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity as Keys, ZwlrLayerSurfaceV1,
};
use rikka_wayland_client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use x11rb::connection::Connection as _;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    ButtonPressEvent, ConnectionExt as _, CreateWindowAux, EventMask, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

use crate::common::client::{Client, Recorded};
use crate::common::input::{A, Composed, P, PRESSED, Q, RELEASED, SUPER, Typed};
use crate::common::shortcuts::Announced;
use crate::common::{BTN_LEFT, Harness, PATIENCE, mapped, shows, tile};

const EXTENT: Extent = Extent {
    width: 320,
    height: 240,
};

const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const SHOWN_BLUE: [u8; 4] = [0, 0, 255, 255];
const SHOWN_RED: [u8; 4] = [255, 0, 0, 255];
const X11_RED: u32 = 0x00ff_0000;
const IDLE_QUIET: Duration = Duration::from_millis(50);
const X11_BLUE: u32 = 0x0000_00ff;
const TILING: [(&str, &str); 1] = [("rikka.windows", "layout = \"tiling\"\n")];

const IDLE_AFTER: u32 = 50;

#[test]
fn tap_focuses_window() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let below = mapped(&mut client, 1, (60, 60), GREEN);
    let above = mapped(&mut client, 2, (60, 60), GREEN);
    let (below, above) = (below.surface.id(), above.surface.id());
    client.until("keyboard focus on newest window", |recorded| {
        recorded.shell.keyboard_on == Some(above)
    });

    harness.inject(&[
        Injected::TouchDown {
            slot: 0,
            x: 52.0,
            y: 52.0,
        },
        Injected::TouchFrame,
        Injected::TouchUp { slot: 0 },
        Injected::TouchFrame,
    ]);
    client.until("keyboard focus on tapped window", |recorded| {
        recorded.shell.keyboard_on == Some(below)
    });
}

#[test]
fn virtual_keyboard_runs_binding_and_types() {
    let harness = Harness::full(EXTENT, &[]);
    let mut client = harness.client();
    let devices = client.devices();
    let window = mapped(&mut client, 1, (60, 60), GREEN);
    let id = window.surface.id();
    client.until("keyboard focus on window", |recorded| {
        recorded.shell.keyboard_on == Some(id)
    });

    let keyboard = client.virtual_keyboard(&devices.seat);
    keyboard.send_key(1, A, PRESSED);
    keyboard.send_key(2, A, RELEASED);
    client.until("typed key", |recorded| recorded.shell.keys.contains(&A));

    keyboard.send_key(3, SUPER, PRESSED);
    keyboard.send_key(4, Q, PRESSED);
    keyboard.send_key(5, Q, RELEASED);
    keyboard.send_key(6, SUPER, RELEASED);
    client.until("close requested", |recorded| {
        recorded.shell.closed.contains(&1)
    });
    assert!(
        !client.received.shell.keys.contains(&Q),
        "bound key not delivered to window"
    );
}

#[test]
fn bindings_run_from_device_and_virtual_keyboard() {
    let harness = Harness::full(EXTENT, &[]);
    let mut announced = Announced::listen();
    let mut client = harness.client();
    let devices = client.devices();
    let output = client.output();
    let keyboard = client.virtual_keyboard(&devices.seat);

    assert_eq!(client.received.shell.keyboard_on, None);
    launches_from_both(&harness, &mut client, &keyboard, &mut announced, "nothing");

    let window = mapped(&mut client, 1, (60, 60), GREEN);
    let window_id = window.surface.id();
    client.until("keyboard focus on window", |recorded| {
        recorded.shell.keyboard_on == Some(window_id)
    });
    launches_from_both(&harness, &mut client, &keyboard, &mut announced, "a window");

    let (panel, layer) = keyed_layer(&mut client, 2, Level::Top, Keys::Exclusive);
    let panel_id = panel.id();
    client.until("keyboard focus on layer surface", |recorded| {
        recorded.shell.keyboard_on == Some(panel_id)
    });
    launches_from_both(&harness, &mut client, &keyboard, &mut announced, "a layer");
    layer.destroy();
    panel.destroy();
    client.until("keyboard focus back on window", |recorded| {
        recorded.shell.keyboard_on == Some(window_id)
    });

    let inhibitor = client.inhibit_shortcuts(&window.surface, &devices.seat);
    super_p(&keyboard, &mut client);
    harness.press(&[SUPER, P]);
    client.until(
        "Super+P delivered to shortcut-inhibiting window",
        |recorded| typed(recorded, P) == 2,
    );
    inhibitor.destroy();

    let lock = client.lock();
    let surface = client.surface();
    let _guard = lock.get_lock_surface(&surface, &output);
    client.until("lock surface configured", |recorded| {
        !recorded.shell.lock_configured.is_empty()
    });
    let pixels = client.pixels(EXTENT.width as i32, EXTENT.height as i32, 3);
    pixels.fill(0..EXTENT.height as i32, BLUE);
    client.show(&surface, &pixels);
    let lock_id = surface.id();
    client.until("keyboard focus on lock surface", |recorded| {
        recorded.shell.locked && recorded.shell.keyboard_on == Some(lock_id)
    });
    super_p(&keyboard, &mut client);
    harness.press(&[SUPER, P]);
    client.until("Super+P delivered to lock surface", |recorded| {
        typed(recorded, P) == 4
    });
    lock.unlock_and_destroy();
    client.roundtrip();
}

#[test]
fn on_demand_layer_returns_focus_on_unmap() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let first = mapped(&mut client, 1, (60, 60), GREEN);
    let second = mapped(&mut client, 2, (60, 60), GREEN);
    let (first, second_id) = (first.surface.id(), second.surface.id());
    client.until("keyboard focus on newest window", |recorded| {
        recorded.shell.keyboard_on == Some(second_id)
    });

    let _backdrop = keyed_layer(&mut client, 3, Level::Background, Keys::OnDemand);
    let (sidebar, sidebar_layer) = keyed_layer(&mut client, 4, Level::Top, Keys::OnDemand);
    let sidebar_id = sidebar.id();
    client.until("keyboard focus on sidebar", |recorded| {
        recorded.shell.keyboard_on == Some(sidebar_id)
    });

    second.toplevel.destroy();
    second.xdg.destroy();
    second.surface.destroy();
    client.roundtrip();
    assert_eq!(
        client.received.shell.keyboard_on,
        Some(sidebar_id),
        "sidebar keeps focus when window moves away"
    );

    let (launcher, launcher_layer) = keyed_layer(&mut client, 5, Level::Overlay, Keys::Exclusive);
    let launcher_id = launcher.id();
    client.until("keyboard focus on launcher", |recorded| {
        recorded.shell.keyboard_on == Some(launcher_id)
    });
    launcher_layer.destroy();
    launcher.destroy();
    client.until("keyboard focus back on sidebar", |recorded| {
        recorded.shell.keyboard_on == Some(sidebar_id)
    });

    sidebar_layer.destroy();
    sidebar.destroy();
    client.until("keyboard focus on remaining window", |recorded| {
        recorded.shell.keyboard_on == Some(first)
    });
}

#[test]
fn on_demand_layer_keeps_focus_over_tiles() {
    let harness = Harness::full(EXTENT, &TILING);
    let mut client = harness.client();
    let _devices = client.devices();
    let window = tile(&mut client, 1, GREEN);
    let window_id = window.surface.id();
    client.until("keyboard focus on tile", |recorded| {
        recorded.shell.keyboard_on == Some(window_id)
    });
    let (sidebar, _sidebar_layer) = keyed_layer(&mut client, 2, Level::Top, Keys::OnDemand);
    let sidebar_id = sidebar.id();
    client.until("keyboard focus on sidebar", |recorded| {
        recorded.shell.keyboard_on == Some(sidebar_id)
    });

    harness.inject(&[Injected::PointerTo { x: 200.0, y: 150.0 }]);
    client.until("pointer over tile", |recorded| {
        recorded.shell.pointer_on == Some(window_id)
    });
    assert_eq!(client.received.shell.keyboard_on, Some(sidebar_id));

    harness.click(200.0, 150.0);
    client.until("keyboard focus on clicked tile", |recorded| {
        recorded.shell.keyboard_on == Some(window_id)
    });
}

#[test]
fn hovered_tile_takes_keyboard_focus() {
    for composing in [false, true] {
        let harness = Harness::full(EXTENT, &TILING);
        let mut client = harness.client();
        let _devices = client.devices();
        let left = tile(&mut client, 1, GREEN).surface.id();
        let right = tile(&mut client, 2, BLUE).surface.id();
        client.until("keyboard focus on newest tile", |recorded| {
            recorded.shell.keyboard_on == Some(right)
        });
        let mut method = harness.client();
        let method_devices = method.devices();
        let _composer = composing.then(|| method.input_method(&method_devices.seat));

        harness.inject(&[Injected::PointerTo { x: 80.0, y: 120.0 }]);
        client.until(
            &format!("keyboard focus on hovered tile (composing: {composing})"),
            |recorded| recorded.shell.keyboard_on == Some(left),
        );
    }
}

#[test]
fn closing_focused_tile_focuses_hovered_tile() {
    let harness = Harness::full(EXTENT, &TILING);
    let mut client = harness.client();
    let _devices = client.devices();
    let left = tile(&mut client, 1, GREEN).surface.id();
    let _upper = tile(&mut client, 2, BLUE);
    let newest = tile(&mut client, 3, GREEN);
    let newest_id = newest.surface.id();
    client.until("newest tile splits the column", |recorded| {
        recorded.shell.keyboard_on == Some(newest_id)
            && recorded
                .shell
                .last_configure(2)
                .is_some_and(|configure| configure.height < 224)
    });
    harness.inject(&[Injected::PointerTo { x: 80.0, y: 120.0 }]);
    client.until("keyboard focus on hovered tile", |recorded| {
        recorded.shell.keyboard_on == Some(left)
    });

    newest.toplevel.destroy();
    newest.xdg.destroy();
    newest.surface.destroy();
    client.until("upper tile fills the column", |recorded| {
        recorded
            .shell
            .last_configure(2)
            .is_some_and(|configure| configure.height == 224)
    });
    assert_eq!(client.received.shell.keyboard_on, Some(left));
}

fn super_p(keyboard: &ZwpVirtualKeyboardV1, client: &mut Client) {
    for (key, state) in [
        (SUPER, PRESSED),
        (P, PRESSED),
        (P, RELEASED),
        (SUPER, RELEASED),
    ] {
        keyboard.send_key(0, key, state);
    }
    client.roundtrip();
}

fn launches_from_both(
    harness: &Harness,
    client: &mut Client,
    keyboard: &ZwpVirtualKeyboardV1,
    announced: &mut Announced,
    holder: &str,
) {
    super_p(keyboard, client);
    assert_eq!(
        announced.next(&format!("virtual Super+P while {holder} has focus")),
        Shortcut::ToggleLauncher
    );
    harness.press(&[SUPER, P]);
    assert_eq!(
        announced.next(&format!("device Super+P while {holder} has focus")),
        Shortcut::ToggleLauncher
    );
}

fn typed(recorded: &Recorded, key: u32) -> usize {
    recorded
        .shell
        .keys
        .iter()
        .filter(|typed_key| **typed_key == key)
        .count()
}

fn keyed_layer(
    client: &mut Client,
    tag: u32,
    level: Level,
    keys: Keys,
) -> (WlSurface, ZwlrLayerSurfaceV1) {
    let (surface, layer) = client.layer_on(level, tag, Anchor::TOP | Anchor::LEFT, (40, 40), 0);
    layer.set_keyboard_interactivity(keys);
    let pixels = client.pixels(40, 40, tag);
    pixels.fill(0..40, BLUE);
    client.show(&surface, &pixels);
    (surface, layer)
}

#[test]
fn virtual_pointer_moves_and_clicks() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let below = mapped(&mut client, 1, (60, 60), GREEN);
    let above = mapped(&mut client, 2, (60, 60), GREEN);
    let (below, above) = (below.surface.id(), above.surface.id());
    client.until("keyboard focus on newest window", |recorded| {
        recorded.shell.keyboard_on == Some(above)
    });

    let pointer = client.virtual_pointer(None);
    pointer.motion_absolute(1, 52, 52, 320, 240);
    pointer.frame();
    pointer.button(2, BTN_LEFT, ButtonState::Pressed);
    pointer.button(3, BTN_LEFT, ButtonState::Released);
    pointer.frame();
    client.until("click delivered to window under pointer", |recorded| {
        recorded.shell.pointer_on == Some(below)
            && recorded.shell.keyboard_on == Some(below)
            && !recorded.shell.pressed.is_empty()
    });
}

#[test]
fn locked_pointer_stays_until_unlocked() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let devices = client.devices();
    let window = mapped(&mut client, 1, (60, 60), GREEN);
    let id = window.surface.id();

    harness.inject(&[Injected::PointerTo { x: 52.0, y: 52.0 }]);
    client.until("pointer focus on window", |recorded| {
        recorded.shell.pointer_on == Some(id)
    });

    let locked = client.lock_pointer(&window.surface, &devices.pointer);
    client.until("pointer locked", |recorded| recorded.input.locked == [true]);

    harness.inject(&[Injected::PointerTo { x: 300.0, y: 220.0 }]);
    client.roundtrip();
    assert_eq!(client.received.shell.pointer_on, Some(id));

    locked.destroy();
    client.roundtrip();
    harness.inject(&[Injected::PointerTo { x: 310.0, y: 230.0 }]);
    client.until("pointer unlocked", |recorded| {
        recorded.shell.pointer_on.is_none()
    });
}

#[test]
fn locked_pointer_reports_relative_motion() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let devices = client.devices();
    let _relative = client.relative_pointer(&devices.pointer);
    let window = mapped(&mut client, 1, (60, 60), GREEN);
    let id = window.surface.id();

    let mouse = client.virtual_pointer(None);
    mouse.motion_absolute(1, 52, 52, EXTENT.width, EXTENT.height);
    mouse.frame();
    client.until("pointer focus on window", |recorded| {
        recorded.shell.pointer_on == Some(id)
    });
    let _locked = client.lock_pointer(&window.surface, &devices.pointer);
    client.until("pointer locked", |recorded| recorded.input.locked == [true]);

    mouse.motion(2, 0.0, -12.0);
    mouse.frame();
    client.until("relative motion delivered", |recorded| {
        recorded.input.travelled.contains(&(0.0, -12.0))
    });
    assert_eq!(client.received.shell.pointer_on, Some(id));
}

#[test]
fn visible_idle_inhibitor_prevents_idle() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let devices = client.devices();
    let window = mapped(&mut client, 1, (60, 60), GREEN);

    let _notification = client.idle_after(&devices.seat, IDLE_AFTER);
    client.until("session idle", |recorded| recorded.input.idle == [true]);

    let inhibitor = client.inhibit_idle(&window.surface);
    harness.inject(&[Injected::PointerTo { x: 10.0, y: 10.0 }]);
    client.until("session resumed by input", |recorded| {
        recorded.input.idle == [true, false]
    });
    std::thread::sleep(Duration::from_millis(u64::from(IDLE_AFTER) * 6));
    client.roundtrip();
    assert_eq!(client.received.input.idle, [true, false]);

    inhibitor.destroy();
    client.until("session idle after inhibitor removed", |recorded| {
        recorded.input.idle == [true, false, true]
    });
}

#[test]
fn agent_click_keeps_user_focus() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let seats = client.seats();
    let person = seats.iter().find(|seated| seated.name == "seat-0").unwrap();
    let agent = seats.iter().find(|seated| seated.name == SEAT).unwrap();
    let (person_tag, agent_tag) = (person.tag, agent.tag);
    let agent_seat = agent.seat.clone();

    let below = mapped(&mut client, 1, (60, 60), GREEN);
    let above = mapped(&mut client, 2, (60, 60), GREEN);
    let (below, above) = (below.surface.id(), above.surface.id());
    client.until("user keyboard focus on newest window", |recorded| {
        recorded.input.keyboard_on(person_tag) == Some(above)
    });

    let pointer = client.virtual_pointer(Some(&agent_seat));
    pointer.motion_absolute(1, 64, 64, 320, 240);
    pointer.frame();
    pointer.button(2, BTN_LEFT, ButtonState::Pressed);
    pointer.button(3, BTN_LEFT, ButtonState::Released);
    pointer.frame();
    client.until("agent keyboard focus on clicked window", |recorded| {
        recorded.input.keyboard_on(agent_tag) == Some(below)
    });
    assert_eq!(client.received.input.keyboard_on(person_tag), Some(above));

    let placement: AgentPlacement = harness.ask(Request::new(CAPABILITY, PLACEMENT));
    assert_eq!((placement.width, placement.height), (320, 240));
    assert!((placement.x - 64.0).abs() < 1.0 && (placement.y - 64.0).abs() < 1.0);
    assert!(!placement.visible);

    let idle: Idle = harness.ask(
        Request::new(CAPABILITY, WAIT_IDLE).encode(&WaitIdleRequest::new(IDLE_QUIET, PATIENCE)),
    );
    assert_eq!(idle, Idle::Quiescent);

    harness.frame_where(move |snapshot| shows(snapshot, 64, 64, GREEN));
    harness.ask::<()>(Request::new(CAPABILITY, SHOW).encode(&true));
    harness.frame_where(move |snapshot| !shows(snapshot, 64, 64, GREEN));
}

#[test]
fn text_input_relays_to_input_method() {
    let harness = Harness::headless(EXTENT);
    let mut app = harness.client();
    let devices = app.devices();
    let pointer = app.located_pointer(&devices.seat);
    let window = mapped(&mut app, 1, (200, 120), GREEN);
    let id = window.surface.id();
    app.until("keyboard focus on window", |recorded| {
        recorded.shell.keyboard_on == Some(id)
    });
    harness.inject(&[Injected::PointerTo { x: 100.0, y: 100.0 }]);
    app.until("pointer focus on window", |recorded| {
        recorded.input.entered_at.is_some()
    });
    let (local_x, local_y) = app.received.input.entered_at.unwrap();
    let origin = ((100.0 - local_x) as i32, (100.0 - local_y) as i32);
    drop(pointer);

    let text_input = app.text_input(&devices.seat);
    let mut method = harness.client();
    let method_devices = method.devices();
    let (input_method, _grab) = method.input_method(&method_devices.seat);
    app.until("text input entered", |recorded| {
        recorded.input.typed.contains(&Typed::Entered)
    });

    text_input.enable();
    text_input.set_cursor_rectangle(12, 30, 2, 18);
    text_input.commit();
    app.roundtrip();
    method.until("input method activated", |recorded| {
        recorded.input.composed == [Composed::Activated, Composed::Done]
    });

    let caret: Option<Caret> = harness.ask(Request::new(Caret::CAPABILITY, Caret::WHERE));
    let typed_at = Caret {
        x: origin.0 + 12,
        y: origin.1 + 30,
        width: 2,
        height: 18,
    };
    assert_eq!(caret, Some(typed_at));

    let aim: Option<PanelAnchor> = harness.ask(Request::new(Caret::CAPABILITY, Caret::AIM));
    assert_eq!(
        aim,
        Some(PanelAnchor::Caret {
            output: "headless-0".to_owned(),
            caret: typed_at,
        }),
        "input popup placed at caret, not pointer"
    );

    input_method.commit_string("かな");
    input_method.commit(1);
    method.roundtrip();
    app.until("committed text", |recorded| {
        recorded
            .input
            .typed
            .ends_with(&[Typed::Committed("かな".to_owned()), Typed::Done])
    });

    harness.press(&[A]);
    method.until("key grabbed by input method", |recorded| {
        recorded.input.grabbed.contains(&A)
    });
    app.roundtrip();
    assert!(!app.received.shell.keys.contains(&A));

    let candidates = method.surface();
    let _popup = input_method.get_input_popup_surface(&candidates);
    let pixels = method.pixels(40, 20, 9);
    pixels.fill(0..20, BLUE);
    method.show(&candidates, &pixels);
    let below_caret = ((origin.0 + 12 + 5) as u32, (origin.1 + 48 + 5) as u32);
    harness.frame_where(move |snapshot| shows(snapshot, below_caret.0, below_caret.1, SHOWN_BLUE));
}

const HOLDING: [SerialKind; 2] = [SerialKind::ButtonPressed, SerialKind::ButtonReleased];

#[derive(Default)]
struct Woken(AtomicBool);

impl Wake for Woken {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn shared_output_keeps_press_serials() {
    let harness = Harness::headless(EXTENT);
    let mut client = harness.client();
    let _devices = client.devices();
    let _window = mapped(&mut client, 1, (60, 60), GREEN);
    let host = Host::with_reader(client.display()).unwrap();
    let serials = host.serials().unwrap();
    client.roundtrip();
    assert_eq!(serials.latest(&HOLDING), None);
    client.roundtrip();
    assert!(serials.latest(&[SerialKind::KeyboardEntered]).is_some());

    harness.inject(&[
        Injected::PointerTo { x: 52.0, y: 52.0 },
        Injected::Button {
            code: BTN_LEFT,
            pressed: true,
        },
    ]);
    client.until("button press delivered", |recorded| {
        !recorded.shell.pressed.is_empty()
    });
    let pressed = Some(Serial {
        serial: *client.received.shell.pressed.last().unwrap(),
        kind: SerialKind::ButtonPressed,
    });
    assert_eq!(serials.latest(&HOLDING), pressed);

    harness.inject(&[Injected::PointerTo { x: 40.0, y: 30.0 }]);
    client.roundtrip();
    assert_eq!(serials.latest(&HOLDING), pressed);

    let woken = Arc::new(Woken::default());
    serials.register_waker(SerialKind::ButtonReleased, Waker::from(Arc::clone(&woken)));
    harness.inject(&[Injected::Button {
        code: BTN_LEFT,
        pressed: false,
    }]);
    client.roundtrip();
    let released = serials.latest(&HOLDING).unwrap();
    assert_eq!(released.kind, SerialKind::ButtonReleased);
    assert_ne!(Some(released), pressed);
    assert!(woken.0.load(Ordering::SeqCst));

    harness.press(&[A]);
    client.roundtrip();
    let acted = [SerialKind::ButtonPressed, SerialKind::KeyPressed];
    assert_eq!(
        serials.latest(&acted).map(|latest| latest.kind),
        Some(SerialKind::KeyPressed)
    );
}

fn x11_window(
    x11: &RustConnection,
    screen: usize,
    at: (i16, i16),
    size: (u16, u16),
    pixel: u32,
    override_redirect: bool,
) -> Window {
    let root = &x11.setup().roots[screen];
    let window = x11.generate_id().unwrap();
    x11.create_window(
        root.root_depth,
        window,
        root.root,
        at.0,
        at.1,
        size.0,
        size.1,
        0,
        WindowClass::INPUT_OUTPUT,
        root.root_visual,
        &CreateWindowAux::new()
            .background_pixel(pixel)
            .override_redirect(u32::from(override_redirect))
            .event_mask(EventMask::BUTTON_PRESS),
    )
    .unwrap();
    x11.map_window(window).unwrap();
    x11.flush().unwrap();
    window
}

fn next_x11_press(x11: &RustConnection) -> ButtonPressEvent {
    loop {
        if let Event::ButtonPress(press) = x11.wait_for_event().expect("X11 connection stays open")
        {
            return press;
        }
    }
}

#[test]
fn click_reaches_x11_menu_opened_during_press() {
    let harness = Harness::headless(EXTENT);
    let (x11, screen) = harness.x11();
    let page = x11_window(&x11, screen, (0, 0), (200, 150), X11_RED, false);
    harness.frame_where(move |snapshot| shows(snapshot, 100, 100, SHOWN_RED));
    let placed = x11
        .translate_coordinates(page, x11.setup().roots[screen].root, 0, 0)
        .unwrap()
        .reply()
        .unwrap();
    let (x, y) = (f64::from(placed.dst_x), f64::from(placed.dst_y));
    harness.inject(&[
        Injected::PointerTo {
            x: x + 10.0,
            y: y + 10.0,
        },
        Injected::Button {
            code: BTN_LEFT,
            pressed: true,
        },
    ]);
    assert_eq!(next_x11_press(&x11).event, page);

    let menu_at = (placed.dst_x + 40, placed.dst_y + 40);
    let menu = x11_window(&x11, screen, menu_at, (80, 60), X11_BLUE, true);
    let inside = (menu_at.0 + 40, menu_at.1 + 30);
    let shown = (
        u32::from(inside.0.unsigned_abs()),
        u32::from(inside.1.unsigned_abs()),
    );
    harness.frame_where(move |snapshot| shows(snapshot, shown.0, shown.1, SHOWN_BLUE));
    harness.inject(&[Injected::Button {
        code: BTN_LEFT,
        pressed: false,
    }]);
    harness.click(f64::from(inside.0), f64::from(inside.1));

    let press = next_x11_press(&x11);
    assert_eq!(press.event, menu);
    assert_eq!((press.root_x, press.root_y), inside);
}
