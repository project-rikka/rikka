use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::Instant;

use rikka_wayland_wire::{Connection, DISPLAY_ID, Fill, Writer};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

use super::PATIENCE;

const SYNC: u16 = 0;
const GET_REGISTRY: u16 = 1;
const ERROR: u16 = 0;
const DONE: u16 = 0;
const GLOBAL: u16 = 0;
const BIND: u16 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Event {
    pub(crate) object: u32,
    pub(crate) opcode: u16,
    pub(crate) body: Vec<u8>,
}

impl Event {
    pub(crate) fn word(&self, index: usize) -> u32 {
        let start = index * 4;
        u32::from_ne_bytes(self.body[start..start + 4].try_into().unwrap())
    }

    pub(crate) fn string(&self, index: usize) -> String {
        let length = self.word(index) as usize;
        let start = (index + 1) * 4;
        String::from_utf8(self.body[start..start + length - 1].to_vec()).unwrap()
    }
}

pub(crate) struct RawClient {
    connection: Connection,
    next_id: u32,
}

impl RawClient {
    pub(crate) fn new(stream: UnixStream) -> Self {
        Self {
            connection: Connection::new(stream),
            next_id: 2,
        }
    }

    pub(crate) fn allocate(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(crate) fn send(&mut self, object: u32, opcode: u16, build: impl FnOnce(&mut Writer<'_>)) {
        let mut writer = self.connection.write(object, opcode);
        build(&mut writer);
        writer.finish().unwrap();
        self.connection.flush().unwrap();
    }

    pub(crate) fn send_bytes(&mut self, bytes: &[u8]) {
        let socket = self.connection.socket().unwrap();
        rustix::io::write(&*socket, bytes).unwrap();
    }

    pub(crate) fn send_fds(&mut self, fds: &[BorrowedFd<'_>]) {
        let callback = self.allocate();
        let bytes: Vec<u8> = [DISPLAY_ID, 12 << 16, callback]
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let mut space =
            vec![MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(fds.len()))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        assert!(control.push(SendAncillaryMessage::ScmRights(fds)));
        let socket = self.connection.socket().unwrap();
        sendmsg(
            &*socket,
            &[IoSlice::new(&bytes)],
            &mut control,
            SendFlags::empty(),
        )
        .unwrap();
    }

    pub(crate) fn next(&mut self) -> Option<Event> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if self.connection.peek().unwrap().is_some() {
                let message = self.connection.next_message().unwrap().unwrap();
                let (object, opcode) = (message.object(), message.opcode());
                let body = message.body().to_vec();
                return Some(Event {
                    object,
                    opcode,
                    body,
                });
            }
            match self.connection.fill() {
                Ok(Fill::Read(_)) => continue,
                Ok(Fill::Closed) | Err(_) => return None,
                Ok(Fill::WouldBlock | Fill::Full) => {}
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "the display went quiet");
            let socket = self.connection.socket().unwrap();
            let mut fds = [PollFd::from_borrowed_fd(socket.as_fd(), PollFlags::IN)];
            let timeout = Timespec {
                tv_sec: left.as_secs() as i64,
                tv_nsec: i64::from(left.subsec_nanos()),
            };
            poll(&mut fds, Some(&timeout)).unwrap();
        }
    }

    pub(crate) fn roundtrip(&mut self) -> Vec<Event> {
        let callback = self.allocate();
        self.send(DISPLAY_ID, SYNC, |writer| {
            writer.object_id(callback);
        });
        let mut events = Vec::new();
        loop {
            let event = self.next().expect("the display hung up during a roundtrip");
            if event.object == callback && event.opcode == DONE {
                return events;
            }
            events.push(event);
        }
    }

    pub(crate) fn registry(&mut self) -> (u32, Vec<(u32, String, u32)>) {
        let registry = self.allocate();
        self.send(DISPLAY_ID, GET_REGISTRY, |writer| {
            writer.object_id(registry);
        });
        let globals = self
            .roundtrip()
            .into_iter()
            .filter(|event| event.object == registry && event.opcode == GLOBAL)
            .map(|event| {
                let interface = event.string(1);
                let padded = (interface.len() + 1).div_ceil(4);
                (event.word(0), interface, event.word(2 + padded))
            })
            .collect();
        (registry, globals)
    }

    pub(crate) fn bind(&mut self, registry: u32, name: u32, interface: &str, version: u32) -> u32 {
        let id = self.allocate();
        self.send(registry, BIND, |writer| {
            writer
                .uint(name)
                .string(interface)
                .uint(version)
                .object_id(id);
        });
        id
    }

    pub(crate) fn error(&mut self) -> (u32, u32, String) {
        loop {
            let event = self.next().expect("the display hung up without an error");
            if event.object == DISPLAY_ID && event.opcode == ERROR {
                return (event.word(0), event.word(1), event.string(2));
            }
        }
    }

    pub(crate) fn await_hangup(&mut self) {
        while self.next().is_some() {}
    }
}
