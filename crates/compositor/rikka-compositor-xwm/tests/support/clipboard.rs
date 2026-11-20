use std::os::fd::OwnedFd;

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, PropMode, Property,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::support::x11::{atom, connection_fd, drain, window};

const CHUNK: usize = 16 * 1024;

struct Sending {
    requestor: Window,
    property: Atom,
    offset: usize,
}

pub(crate) struct Owner {
    conn: RustConnection,
    data: Vec<u8>,
    targets: Atom,
    utf8: Atom,
    incr: Atom,
    whole_limit: usize,
    sending: Option<Sending>,
}

impl Owner {
    pub(crate) fn claim(conn: RustConnection, data: Vec<u8>) -> Self {
        Self::claim_up_to(conn, data, CHUNK)
    }

    pub(crate) fn claim_whole(conn: RustConnection, data: Vec<u8>) -> Self {
        Self::claim_up_to(conn, data, usize::MAX)
    }

    fn claim_up_to(conn: RustConnection, data: Vec<u8>, whole_limit: usize) -> Self {
        let window = window(&conn, false, EventMask::NO_EVENT);
        conn.set_selection_owner(window, atom(&conn, "CLIPBOARD"), x11rb::CURRENT_TIME)
            .unwrap();
        Self {
            targets: atom(&conn, "TARGETS"),
            utf8: atom(&conn, "UTF8_STRING"),
            incr: atom(&conn, "INCR"),
            conn,
            data,
            whole_limit,
            sending: None,
        }
    }

    pub(crate) fn replace(&mut self, data: Vec<u8>) {
        self.data = data;
    }

    pub(crate) fn watch_fd(&self) -> OwnedFd {
        connection_fd(&self.conn)
    }

    pub(crate) fn service(&mut self) {
        loop {
            let events = drain(&self.conn);
            if events.is_empty() {
                break;
            }
            for event in events {
                match event {
                    Event::SelectionRequest(request) => self.answer(&request),
                    Event::PropertyNotify(change) if change.state == Property::DELETE => {
                        self.continue_with(change.window, change.atom);
                    }
                    _ => {}
                }
            }
        }
        self.conn.flush().unwrap();
    }

    fn answer(&mut self, request: &SelectionRequestEvent) {
        let property = if request.property == x11rb::NONE {
            request.target
        } else {
            request.property
        };
        let conn = &self.conn;
        if request.target == self.targets {
            conn.change_property32(
                PropMode::REPLACE,
                request.requestor,
                property,
                AtomEnum::ATOM,
                &[self.targets, self.utf8],
            )
            .unwrap();
        } else if request.target == self.utf8 && self.data.len() <= self.whole_limit {
            conn.change_property8(
                PropMode::REPLACE,
                request.requestor,
                property,
                self.utf8,
                &self.data,
            )
            .unwrap();
        } else if request.target == self.utf8 {
            conn.change_window_attributes(
                request.requestor,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
            )
            .unwrap();
            conn.change_property32(
                PropMode::REPLACE,
                request.requestor,
                property,
                self.incr,
                &[u32::try_from(self.data.len()).unwrap()],
            )
            .unwrap();
            self.sending = Some(Sending {
                requestor: request.requestor,
                property,
                offset: 0,
            });
        } else {
            notify(conn, request, x11rb::NONE);
            return;
        }
        notify(conn, request, property);
    }

    fn continue_with(&mut self, window: Window, property: Atom) {
        let Some(sending) = self.sending.as_mut() else {
            return;
        };
        if sending.requestor != window || sending.property != property {
            return;
        }
        let end = (sending.offset + CHUNK).min(self.data.len());
        self.conn
            .change_property8(
                PropMode::REPLACE,
                window,
                property,
                self.utf8,
                &self.data[sending.offset..end],
            )
            .unwrap();
        if sending.offset == end {
            self.sending = None;
        } else {
            sending.offset = end;
        }
    }
}

fn notify(conn: &RustConnection, request: &SelectionRequestEvent, property: Atom) {
    conn.send_event(
        false,
        request.requestor,
        EventMask::NO_EVENT,
        SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property,
        },
    )
    .unwrap();
}

pub(crate) struct Requestor {
    conn: RustConnection,
    window: Window,
    property: Atom,
    incr: Atom,
    buffer: Vec<u8>,
    incremental: bool,
    pub(crate) chunked: bool,
    pub(crate) received: Option<Result<Vec<u8>, ()>>,
}

impl Requestor {
    pub(crate) fn new(conn: RustConnection) -> Self {
        let window = window(&conn, false, EventMask::PROPERTY_CHANGE);
        Self {
            property: atom(&conn, "RIKKA_PASTE"),
            incr: atom(&conn, "INCR"),
            conn,
            window,
            buffer: Vec::new(),
            incremental: false,
            chunked: false,
            received: None,
        }
    }

    pub(crate) fn ask(&mut self, target: &str) {
        self.received = None;
        self.buffer.clear();
        self.incremental = false;
        self.chunked = false;
        self.conn
            .convert_selection(
                self.window,
                atom(&self.conn, "CLIPBOARD"),
                atom(&self.conn, target),
                self.property,
                x11rb::CURRENT_TIME,
            )
            .unwrap();
        self.conn.flush().unwrap();
    }

    pub(crate) fn atoms(&self) -> Vec<Atom> {
        let bytes = self.received.clone().unwrap().unwrap();
        let (words, _) = bytes.as_chunks::<4>();
        words.iter().copied().map(u32::from_ne_bytes).collect()
    }

    pub(crate) fn atom(&self, name: &str) -> Atom {
        atom(&self.conn, name)
    }

    pub(crate) fn watch_fd(&self) -> OwnedFd {
        connection_fd(&self.conn)
    }

    pub(crate) fn service(&mut self) {
        loop {
            let events = drain(&self.conn);
            if events.is_empty() {
                break;
            }
            for event in events {
                self.handle(event);
            }
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::SelectionNotify(notify) if notify.requestor == self.window => {
                if notify.property == x11rb::NONE {
                    self.received = Some(Err(()));
                    return;
                }
                let reply = self.read();
                if reply.0 == self.incr {
                    self.incremental = true;
                    self.chunked = true;
                } else {
                    self.received = Some(Ok(reply.1));
                }
            }
            Event::PropertyNotify(change)
                if change.window == self.window
                    && change.atom == self.property
                    && change.state == Property::NEW_VALUE
                    && self.incremental =>
            {
                let (_, value) = self.read();
                if value.is_empty() {
                    self.incremental = false;
                    self.received = Some(Ok(std::mem::take(&mut self.buffer)));
                } else {
                    self.buffer.extend(value);
                }
            }
            _ => {}
        }
    }

    fn read(&self) -> (Atom, Vec<u8>) {
        let reply = self
            .conn
            .get_property(
                true,
                self.window,
                self.property,
                AtomEnum::ANY,
                0,
                u32::MAX / 4,
            )
            .unwrap()
            .reply()
            .unwrap();
        (reply.type_, reply.value)
    }
}
