use std::io::Read;
use std::io::Write;
use std::os::fd::{AsFd, OwnedFd};
use std::rc::Rc;

use rikka_compositor_xwm::{
    Error, FrameExtents, Geometry, MimeList, Protocols, SelectionKind, WindowProperty, WindowType,
    XSetting, XWindowId, XwmEvent,
};
use rikka_types::{DragAction, ResizeEdge};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::pipe::{PipeFlags, pipe_with};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask, PropMode,
};
use x11rb::wrapper::ConnectionExt as _;

use crate::support::clipboard::{Owner, Requestor};
use crate::support::session::{Session, wait_until};
use crate::support::x11::{atom, drain, root, set_atoms, set_text, watch, window, words};

const UTF8: &str = "text/plain;charset=utf-8";

#[test]
fn x11_windows_are_managed() {
    let mut session = Session::start();
    let app = session.client();
    let app_window = window(
        &app,
        false,
        EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE | EventMask::FOCUS_CHANGE,
    );
    watch(
        &app,
        root(&app),
        EventMask::PROPERTY_CHANGE | EventMask::SUBSTRUCTURE_NOTIFY,
    );
    set_text(&app, app_window, "_NET_WM_NAME", "Rikka 日本語");
    app.change_property8(
        PropMode::REPLACE,
        app_window,
        AtomEnum::WM_CLASS,
        AtomEnum::STRING,
        b"rikka-test\0RikkaTest\0",
    )
    .unwrap();
    set_atoms(&app, app_window, "WM_PROTOCOLS", &["WM_DELETE_WINDOW"]);
    set_atoms(
        &app,
        app_window,
        "_NET_WM_WINDOW_TYPE",
        &["_NET_WM_WINDOW_TYPE_DIALOG"],
    );
    app.map_window(app_window).unwrap();
    drain(&app);

    let id = map_requested(&mut session, app_window);
    let known = session
        .xwm
        .window(id)
        .expect("the window is tracked")
        .clone();
    assert_eq!(known.title, "Rikka 日本語");
    assert_eq!(known.instance, "rikka-test");
    assert_eq!(known.class, "RikkaTest");
    assert!(known.protocols.contains(Protocols::DELETE_WINDOW));
    assert_eq!(known.window_type(), WindowType::Dialog);
    assert_eq!(known.pid, Some(std::process::id()));
    assert!(!known.override_redirect);

    let shape = Geometry::new(50, 60, 300, 200);
    let serial = session.xwm.configure(id, shape).unwrap();
    session.xwm.map(id).unwrap();
    session.wait_for("the configure to be accepted", |event| {
        matches!(event, XwmEvent::ConfigureAccepted { window, serial: accepted, geometry }
            if *window == id && *accepted == serial && *geometry == shape)
    });
    session.wait_for(
        "the window to map",
        |event| matches!(event, XwmEvent::Mapped { window } if *window == id),
    );
    assert_eq!(session.xwm.window(id).unwrap().geometry, shape);
    let again = session.xwm.configure(id, shape).unwrap();
    session.wait_for(
        "an unchanged configure to be accepted",
        |event| matches!(event, XwmEvent::ConfigureAccepted { serial, .. } if *serial == again),
    );

    let XwmEvent::SurfaceSerial { serial, .. } = session.wait_for(
        "the surface serial",
        |event| matches!(event, XwmEvent::SurfaceSerial { window, .. } if *window == id),
    ) else {
        unreachable!()
    };
    assert_eq!(session.xwm.window_for_serial(serial), Some(id));
    session.wait_for_serial(serial);

    app.configure_window(app_window, &ConfigureWindowAux::new().width(420))
        .unwrap();
    drain(&app);
    session.wait_for("a configure request", |event| {
        matches!(event, XwmEvent::ConfigureRequest { window, width: Some(420), height: None, x: None, y: None, .. }
            if *window == id)
    });

    let net_state = atom(&app, "_NET_WM_STATE");
    let fullscreen = atom(&app, "_NET_WM_STATE_FULLSCREEN");
    app.send_event(
        false,
        root(&app),
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        ClientMessageEvent::new(32, app_window, net_state, [1, fullscreen, 0, 1, 0]),
    )
    .unwrap();
    drain(&app);
    session.wait_for("a fullscreen request", |event| {
        matches!(event, XwmEvent::FullscreenRequest { window, fullscreen: true } if *window == id)
    });
    session.xwm.set_fullscreen(id, true).unwrap();
    session.flush();
    wait_until(&app, "the fullscreen state to be published", |_| {
        words(&app, app_window, "_NET_WM_STATE").contains(&fullscreen)
    });
    assert!(session.xwm.window(id).unwrap().is_fullscreen());

    let ask = |kind: &str, data: [u32; 5]| {
        app.send_event(
            false,
            root(&app),
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            ClientMessageEvent::new(32, app_window, atom(&app, kind), data),
        )
        .unwrap();
        drain(&app);
    };
    ask("WM_CHANGE_STATE", [3, 0, 0, 0, 0]);
    session.wait_for("a minimize request", |event| {
        matches!(event, XwmEvent::MinimizeRequest { window, minimized: true } if *window == id)
    });
    session.xwm.set_minimized(id, true).unwrap();
    session.flush();
    wait_until(&app, "the window to be iconic", |_| {
        words(&app, app_window, "WM_STATE").first() == Some(&3)
    });
    session.xwm.set_minimized(id, false).unwrap();
    session.flush();
    wait_until(&app, "the window to be normal again", |_| {
        words(&app, app_window, "WM_STATE").first() == Some(&1)
    });
    ask("_NET_WM_MOVERESIZE", [70, 80, 8, 1, 1]);
    session.wait_for(
        "a move request",
        |event| matches!(event, XwmEvent::MoveRequest { window, button: 1 } if *window == id),
    );
    ask("_NET_WM_MOVERESIZE", [70, 80, 4, 1, 1]);
    session.wait_for("a resize request", |event| {
        matches!(event, XwmEvent::ResizeRequest { window, button: 1, edge: ResizeEdge::BottomRight }
            if *window == id)
    });
    ask("_NET_REQUEST_FRAME_EXTENTS", [0; 5]);
    session.wait_for(
        "a frame extents request",
        |event| matches!(event, XwmEvent::FrameExtentsRequest { window } if *window == id),
    );
    let extents = FrameExtents {
        top: 39,
        ..FrameExtents::default()
    };
    session.xwm.set_frame_extents(id, extents).unwrap();
    session.flush();
    wait_until(&app, "the frame extents to be published", |_| {
        words(&app, app_window, "_NET_FRAME_EXTENTS") == [0, 0, 39, 0]
    });

    let xsettings_manager = app
        .get_selection_owner(atom(&app, "_XSETTINGS_S0"))
        .unwrap()
        .reply()
        .unwrap()
        .owner;
    watch(&app, xsettings_manager, EventMask::PROPERTY_CHANGE);
    session
        .xwm
        .set_xsetting("Gtk/IMModule", XSetting::String("xim".into()))
        .unwrap();
    session.flush();
    wait_until(&app, "the X settings to carry the input method", |_| {
        let manager = app
            .get_selection_owner(atom(&app, "_XSETTINGS_S0"))
            .unwrap()
            .reply()
            .unwrap()
            .owner;
        let settings = app
            .get_property(
                false,
                manager,
                atom(&app, "_XSETTINGS_SETTINGS"),
                AtomEnum::ANY,
                0,
                1024,
            )
            .unwrap()
            .reply()
            .unwrap()
            .value;
        settings.windows(12).any(|name| name == b"Gtk/IMModule")
            && settings.windows(3).any(|value| value == b"xim")
    });
    assert!(matches!(
        session.xwm.set_xsetting("Gtk/3D", XSetting::Integer(1)),
        Err(Error::SettingName(_))
    ));

    set_text(&app, app_window, "_NET_WM_NAME", "Renamed");
    drain(&app);
    session.wait_for("a title change", |event| {
        matches!(event, XwmEvent::PropertyChanged { window, property: WindowProperty::Title } if *window == id)
    });
    assert_eq!(session.xwm.window(id).unwrap().title, "Renamed");

    session.xwm.focus(Some(id)).unwrap();
    session.flush();
    wait_until(&app, "the X server to give the window the keyboard", |_| {
        app.get_input_focus().unwrap().reply().unwrap().focus == app_window
    });
    wait_until(&app, "the active window to be published", |_| {
        words(&app, root(&app), "_NET_ACTIVE_WINDOW") == [app_window]
    });

    let other_window = window(&app, false, EventMask::NO_EVENT);
    app.map_window(other_window).unwrap();
    drain(&app);
    let other = map_requested(&mut session, other_window);
    session.xwm.map(other).unwrap();
    session.wait_for(
        "the second window to map",
        |event| matches!(event, XwmEvent::Mapped { window } if *window == other),
    );
    assert_eq!(session.xwm.stacking(), [id, other]);
    session.xwm.restack([other, id]).unwrap();
    session.flush();
    assert_eq!(session.xwm.stacking(), [other, id]);
    wait_until(&app, "the stacking order to be published", |_| {
        words(&app, root(&app), "_NET_CLIENT_LIST_STACKING") == [other_window, app_window]
    });
    wait_until(&app, "the client list to be published", |_| {
        words(&app, root(&app), "_NET_CLIENT_LIST") == [app_window, other_window]
    });
    wait_until(&app, "the X server to restack the windows", |_| {
        let children = app
            .query_tree(root(&app))
            .unwrap()
            .reply()
            .unwrap()
            .children;
        let position = |raw| children.iter().position(|child| *child == raw);
        position(other_window) < position(app_window)
    });

    session.xwm.close(id).unwrap();
    session.flush();
    let delete_window = atom(&app, "WM_DELETE_WINDOW");
    wait_until(&app, "the window to be asked to close", |events| {
        events.iter().any(|event| {
            matches!(event, Event::ClientMessage(message)
                if message.window == app_window && message.data.as_data32()[0] == delete_window)
        })
    });

    app.destroy_window(other_window).unwrap();
    drain(&app);
    session.wait_for(
        "the second window to unmap",
        |event| matches!(event, XwmEvent::Unmapped { window } if *window == other),
    );
    session.wait_for(
        "the second window to go",
        |event| matches!(event, XwmEvent::Destroyed { window } if *window == other),
    );
    assert!(session.xwm.window(other).is_none());
    assert_eq!(session.xwm.stacking(), [id]);

    let menu_window = window(&app, true, EventMask::NO_EVENT);
    app.map_window(menu_window).unwrap();
    drain(&app);
    let XwmEvent::Mapped { window: menu } = session.wait_for(
        "an override-redirect window to map",
        |event| matches!(event, XwmEvent::Mapped { window } if window.raw() == menu_window),
    ) else {
        unreachable!()
    };
    assert!(session.xwm.window(menu).unwrap().override_redirect);
    assert!(matches!(
        session.xwm.configure(menu, shape),
        Err(Error::OverrideRedirect(_))
    ));
}

#[test]
fn selections_bridge_x11_and_wayland() {
    let mut session = Session::start();

    let short = b"hello from x11".to_vec();
    let mut owner = Owner::claim(session.client(), short.clone());
    let owner_fd = owner.watch_fd();
    let XwmEvent::SelectionOffered { mime_types, .. } = session.wait_for_serving(
        "an X11 clipboard offer",
        &[owner_fd.as_fd()],
        &mut || owner.service(),
        |event| {
            matches!(
                event,
                XwmEvent::SelectionOffered {
                    kind: SelectionKind::Clipboard,
                    ..
                }
            )
        },
    ) else {
        unreachable!()
    };
    assert_eq!(*mime_types, [Box::<str>::from(UTF8)]);

    assert_eq!(paste_from_x11(&mut session, &mut owner), short);
    let long = pattern(300 * 1024);
    owner.replace(long.clone());
    assert_eq!(paste_from_x11(&mut session, &mut owner), long);
    assert!(matches!(
        session
            .xwm
            .request_selection(SelectionKind::Clipboard, "image/png", pipe().1),
        Err(Error::NotOffered { .. })
    ));

    let app = session.client();
    let focus_window = window(&app, false, EventMask::NO_EVENT);
    app.map_window(focus_window).unwrap();
    drain(&app);
    let focus = map_requested(&mut session, focus_window);
    session.xwm.map(focus).unwrap();
    session.xwm.focus(Some(focus)).unwrap();

    session
        .xwm
        .set_selection(SelectionKind::Clipboard, utf8())
        .unwrap();
    session.flush();
    let mut requestor = Requestor::new(session.client());
    let requestor_fd = requestor.watch_fd();
    requestor.ask("TARGETS");
    session.run_until("the targets", &[requestor_fd.as_fd()], |_| {
        requestor.service();
        requestor.received.is_some()
    });
    let targets = requestor.atoms();
    for name in ["TARGETS", "TIMESTAMP", "UTF8_STRING", "TEXT"] {
        assert!(targets.contains(&requestor.atom(name)), "{name} is missing");
    }

    for (data, chunked) in [
        (b"hello from wayland".to_vec(), false),
        (pattern(200 * 1024), false),
        (pattern(3 * 1024 * 1024 + 17), true),
    ] {
        requestor.ask("UTF8_STRING");
        let XwmEvent::SendSelection { mime_type, fd, .. } = session.wait_for_serving(
            "a request for Wayland data",
            &[requestor_fd.as_fd()],
            &mut || requestor.service(),
            |event| matches!(event, XwmEvent::SendSelection { .. }),
        ) else {
            unreachable!()
        };
        assert_eq!(mime_type, UTF8);
        let writer = write_in_background(fd, data.clone());
        session.run_until("the X11 paste", &[requestor_fd.as_fd()], |_| {
            requestor.service();
            requestor.received.is_some()
        });
        writer.join().unwrap();
        assert_eq!(requestor.received.clone().unwrap().unwrap(), data);
        assert_eq!(requestor.chunked, chunked, "{} bytes", data.len());
    }

    session.xwm.focus(None).unwrap();
    requestor.ask("UTF8_STRING");
    session.run_until(
        "an unfocused paste to be refused",
        &[requestor_fd.as_fd()],
        |_| {
            requestor.service();
            requestor.received.is_some()
        },
    );
    assert_eq!(requestor.received, Some(Err(())));
}

#[test]
fn large_x11_selection_streams_to_wayland() {
    let mut session = Session::start();
    let large = pattern(6 * 1024 * 1024 + 3);
    let mut owner = Owner::claim_whole(session.client(), large.clone());
    let owner_fd = owner.watch_fd();
    session.wait_for_serving(
        "an X11 clipboard offer",
        &[owner_fd.as_fd()],
        &mut || owner.service(),
        |event| {
            matches!(
                event,
                XwmEvent::SelectionOffered {
                    kind: SelectionKind::Clipboard,
                    ..
                }
            )
        },
    );
    assert_eq!(paste_from_x11(&mut session, &mut owner), large);
    assert_eq!(paste_from_x11(&mut session, &mut owner), large);
}

#[test]
fn drag_enters_xdnd_aware_window() {
    let mut session = Session::start();
    let app = session.client();
    let aware_window = window(&app, false, EventMask::NO_EVENT);
    set_word(&app, aware_window, "XdndAware", 5);
    app.map_window(aware_window).unwrap();
    let plain_window = window(&app, false, EventMask::NO_EVENT);
    app.map_window(plain_window).unwrap();
    drain(&app);
    let aware = map_requested(&mut session, aware_window);
    let plain = map_requested(&mut session, plain_window);

    session
        .xwm
        .drag_enter(aware, utf8(), (5, 5), DragAction::Copy)
        .unwrap();
    session.flush();
    let enter = atom(&app, "XdndEnter");
    let position = atom(&app, "XdndPosition");
    let mut messages = Vec::new();
    wait_until(&app, "the aware window to hear the drag", |events| {
        messages.extend(xdnd_messages(events, aware_window));
        messages.len() >= 2
    });
    assert_eq!(messages[0].0, enter);
    assert_eq!(messages[0].1[1] >> 24, 5);
    assert_eq!(messages[1].0, position);
    session.xwm.drag_leave().unwrap();

    session
        .xwm
        .drag_enter(plain, utf8(), (5, 5), DragAction::Copy)
        .unwrap();
    session.flush();
    session.xwm.drag_leave().unwrap();
    session
        .xwm
        .drag_enter(aware, utf8(), (5, 5), DragAction::Copy)
        .unwrap();
    session.flush();
    let mut plain_heard = Vec::new();
    wait_until(&app, "the aware window to hear the next drag", |events| {
        plain_heard.extend(xdnd_messages(events, plain_window));
        xdnd_messages(events, aware_window)
            .iter()
            .any(|(kind, _)| *kind == enter)
    });
    assert!(plain_heard.is_empty());
    session.xwm.drag_leave().unwrap();

    set_word(&app, plain_window, "XdndAware", 5);
    drain(&app);
    let mut plain_heard = Vec::new();
    session.run_until(
        "the window to become aware",
        &[app.stream().as_fd()],
        |xwm| {
            xwm.drag_enter(plain, utf8(), (5, 5), DragAction::Copy)
                .unwrap();
            xwm.flush().unwrap();
            plain_heard.extend(xdnd_messages(&drain(&app), plain_window));
            plain_heard.iter().any(|(kind, _)| *kind == enter)
        },
    );
}

fn set_word(
    conn: &x11rb::rust_connection::RustConnection,
    window: u32,
    property: &str,
    value: u32,
) {
    conn.change_property32(
        PropMode::REPLACE,
        window,
        atom(conn, property),
        AtomEnum::ATOM,
        &[value],
    )
    .unwrap();
}

fn xdnd_messages(events: &[Event], window: u32) -> Vec<(u32, [u32; 5])> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ClientMessage(message) if message.window == window => {
                Some((message.type_, message.data.as_data32()))
            }
            _ => None,
        })
        .collect()
}

fn paste_from_x11(session: &mut Session, owner: &mut Owner) -> Vec<u8> {
    let (read, write) = pipe();
    session
        .xwm
        .request_selection(SelectionKind::Clipboard, UTF8, write)
        .unwrap();
    let flags = fcntl_getfl(&read).unwrap();
    fcntl_setfl(&read, flags | OFlags::NONBLOCK).unwrap();
    let mut read = std::fs::File::from(read);
    let mut pasted = Vec::new();
    let mut ended = false;
    let owner_fd = owner.watch_fd();
    let read_fd = read.as_fd().try_clone_to_owned().unwrap();
    session.run_until(
        "the Wayland paste",
        &[owner_fd.as_fd(), read_fd.as_fd()],
        |_| {
            owner.service();
            let mut buffer = [0u8; 8192];
            loop {
                match read.read(&mut buffer) {
                    Ok(0) => {
                        ended = true;
                        break;
                    }
                    Ok(length) => pasted.extend_from_slice(&buffer[..length]),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => panic!("the paste pipe failed: {error}"),
                }
            }
            ended
        },
    );
    pasted
}

fn write_in_background(fd: OwnedFd, data: Vec<u8>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("rikka-test-writer".into())
        .spawn(move || {
            let mut file = std::fs::File::from(fd);
            file.write_all(&data).unwrap();
        })
        .unwrap()
}

fn pipe() -> (OwnedFd, OwnedFd) {
    pipe_with(PipeFlags::CLOEXEC).unwrap()
}

fn utf8() -> MimeList {
    Rc::from([Box::from(UTF8)])
}

fn pattern(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

fn map_requested(session: &mut Session, raw: u32) -> XWindowId {
    let XwmEvent::MapRequest { window } = session.wait_for(
        "a map request",
        |event| matches!(event, XwmEvent::MapRequest { window } if window.raw() == raw),
    ) else {
        unreachable!()
    };
    window
}
