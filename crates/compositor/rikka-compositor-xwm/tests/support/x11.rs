use std::os::fd::{AsFd, OwnedFd};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux, EventMask,
    PropMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub(crate) fn atom(conn: &RustConnection, name: &str) -> Atom {
    conn.intern_atom(false, name.as_bytes())
        .unwrap()
        .reply()
        .unwrap()
        .atom
}

pub(crate) fn root(conn: &RustConnection) -> Window {
    conn.setup().roots[0].root
}

pub(crate) fn window(conn: &RustConnection, override_redirect: bool, events: EventMask) -> Window {
    let window = conn.generate_id().unwrap();
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        window,
        root(conn),
        10,
        10,
        100,
        80,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new()
            .override_redirect(u32::from(override_redirect))
            .event_mask(events),
    )
    .unwrap();
    window
}

pub(crate) fn set_text(conn: &RustConnection, window: Window, property: &str, value: &str) {
    let utf8 = atom(conn, "UTF8_STRING");
    conn.change_property8(
        PropMode::REPLACE,
        window,
        atom(conn, property),
        utf8,
        value.as_bytes(),
    )
    .unwrap();
}

pub(crate) fn set_atoms(conn: &RustConnection, window: Window, property: &str, values: &[&str]) {
    let values: Vec<Atom> = values.iter().map(|name| atom(conn, name)).collect();
    conn.change_property32(
        PropMode::REPLACE,
        window,
        atom(conn, property),
        AtomEnum::ATOM,
        &values,
    )
    .unwrap();
}

pub(crate) fn words(conn: &RustConnection, window: Window, property: &str) -> Vec<u32> {
    conn.get_property(false, window, atom(conn, property), AtomEnum::ANY, 0, 1024)
        .unwrap()
        .reply()
        .unwrap()
        .value32()
        .map(Iterator::collect)
        .unwrap_or_default()
}

pub(crate) fn watch(conn: &RustConnection, window: Window, events: EventMask) {
    conn.change_window_attributes(window, &ChangeWindowAttributesAux::new().event_mask(events))
        .unwrap();
}

pub(crate) fn connection_fd(conn: &RustConnection) -> OwnedFd {
    conn.stream().as_fd().try_clone_to_owned().unwrap()
}

pub(crate) fn drain(conn: &RustConnection) -> Vec<Event> {
    conn.flush().unwrap();
    std::iter::from_fn(|| conn.poll_for_event().unwrap()).collect()
}
