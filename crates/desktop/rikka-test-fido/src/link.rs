use std::collections::VecDeque;
use std::io;
use std::os::fd::BorrowedFd;
use std::thread;
use std::time::{Duration, Instant};

use rikka_fido::HidLink;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;

use crate::key::{FakeKey, Kind};
use crate::{ctap2, u2f};

const PACKET: usize = 64;
const BROADCAST: u32 = 0xffff_ffff;
const FOREIGN: u32 = 0x0bad_cafe;

const MSG: u8 = 0x03;
const INIT: u8 = 0x06;
const WINK: u8 = 0x08;
const CBOR: u8 = 0x10;
const CANCEL: u8 = 0x11;
const KEEPALIVE: u8 = 0x3b;
const ERROR: u8 = 0x3f;

const ERR_INVALID_CMD: u8 = 0x01;
const ERR_INVALID_SEQ: u8 = 0x04;
const ERR_CHANNEL_BUSY: u8 = 0x06;
const ERR_INVALID_CHANNEL: u8 = 0x0b;

const UP_NEEDED: u8 = 0x02;
const KEEPALIVE_CANCEL: u8 = 0x2d;

const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(10);

pub(crate) enum Reply {
    Now(Vec<u8>),
    AfterTouch(Vec<u8>),
}

struct Assembly {
    cid: u32,
    command: u8,
    length: usize,
    data: Vec<u8>,
    sequence: u8,
}

pub(crate) struct FakeLink {
    key: FakeKey,
    outbox: VecDeque<Vec<u8>>,
    deferred: Option<(Instant, u32, VecDeque<Vec<u8>>)>,
    assembly: Option<Assembly>,
    holding: Option<u32>,
}

impl FakeLink {
    pub(crate) fn new(key: FakeKey) -> FakeLink {
        FakeLink {
            key,
            outbox: VecDeque::new(),
            deferred: None,
            assembly: None,
            holding: None,
        }
    }

    fn dispatch(&mut self, cid: u32, command: u8, data: Vec<u8>) {
        if command == INIT {
            let mut state = self.key.lock();
            let allocated = 0x0100_0000 + state.channels.len() as u32 + 1;
            state.channels.push(allocated);
            let capabilities = match state.kind {
                Kind::U2f => 0x01,
                Kind::Ctap20 | Kind::Ctap21 => 0x05,
            };
            drop(state);
            let mut reply = data[..8.min(data.len())].to_vec();
            reply.extend_from_slice(&allocated.to_be_bytes());
            reply.extend_from_slice(&[2, 5, 0, 0, capabilities]);
            self.queue(cid, INIT, &reply);
            return;
        }

        if !self.key.lock().channels.contains(&cid) {
            self.queue(cid, ERROR, &[ERR_INVALID_CHANNEL]);
            return;
        }

        match command {
            CANCEL => {
                self.key.lock().cancelled = true;
                let deferred = self
                    .deferred
                    .take_if(|(_, waiting, _)| *waiting == cid)
                    .is_some();
                if self.holding.take() == Some(cid) || deferred {
                    self.queue(cid, CBOR, &[KEEPALIVE_CANCEL]);
                }
            }
            WINK => self.queue(cid, WINK, &[]),
            CBOR => {
                let reply = {
                    let mut state = self.key.lock();
                    if state.kind == Kind::U2f {
                        drop(state);
                        self.queue(cid, ERROR, &[ERR_INVALID_CMD]);
                        return;
                    }
                    if state.noisy && !state.busy_sent {
                        state.busy_sent = true;
                        drop(state);
                        self.queue(cid, ERROR, &[ERR_CHANNEL_BUSY]);
                        return;
                    }
                    ctap2::handle(&mut state, &data)
                };
                self.answer(cid, CBOR, reply);
            }
            MSG => {
                let reply = u2f::handle(&mut self.key.lock(), &data);
                self.answer(cid, MSG, reply);
            }
            _ => self.queue(cid, ERROR, &[ERR_INVALID_CMD]),
        }
    }

    fn answer(&mut self, cid: u32, command: u8, reply: Reply) {
        let (noisy, unanswered, touches, delay) = {
            let state = self.key.lock();
            (
                state.noisy,
                state.unanswered,
                state.touches,
                state.touch_delay,
            )
        };

        if noisy {
            self.queue(FOREIGN, command, &[0x00, 0x01, 0x02]);
        }

        match reply {
            Reply::Now(bytes) => self.queue(cid, command, &bytes),
            Reply::AfterTouch(_) if unanswered => self.holding = Some(cid),
            Reply::AfterTouch(bytes) => {
                for _ in 0..touches {
                    self.queue(cid, KEEPALIVE, &[UP_NEEDED]);
                }
                if noisy {
                    let mut stray = vec![0u8; PACKET];
                    stray[..4].copy_from_slice(&cid.to_be_bytes());
                    stray[4] = 0x05;
                    self.outbox.push_back(stray);
                }
                if delay.is_zero() {
                    self.queue(cid, command, &bytes);
                } else {
                    let ready = self.outbox.len();
                    self.queue(cid, command, &bytes);
                    let response = self.outbox.split_off(ready);
                    self.deferred = Some((Instant::now() + delay, cid, response));
                }
            }
        }
    }

    fn queue(&mut self, cid: u32, command: u8, payload: &[u8]) {
        let mut packet = vec![0u8; PACKET];
        packet[..4].copy_from_slice(&cid.to_be_bytes());
        packet[4] = 0x80 | command;
        packet[5..7].copy_from_slice(&(payload.len() as u16).to_be_bytes());
        let first = payload.len().min(PACKET - 7);
        packet[7..7 + first].copy_from_slice(&payload[..first]);
        self.outbox.push_back(packet);

        for (sequence, chunk) in payload[first..].chunks(PACKET - 5).enumerate() {
            let mut packet = vec![0u8; PACKET];
            packet[..4].copy_from_slice(&cid.to_be_bytes());
            packet[4] = sequence as u8;
            packet[5..5 + chunk.len()].copy_from_slice(chunk);
            self.outbox.push_back(packet);
        }
    }
}

impl HidLink for FakeLink {
    fn packet_len(&self) -> usize {
        PACKET
    }

    fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        if packet.len() != PACKET {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let cid = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);

        if packet[4] & 0x80 != 0 {
            let command = packet[4] & 0x7f;
            let length = usize::from(u16::from_be_bytes([packet[5], packet[6]]));
            let data = packet[7..7 + length.min(PACKET - 7)].to_vec();
            self.assembly = Some(Assembly {
                cid,
                command,
                length,
                data,
                sequence: 0,
            });
        } else {
            let Some(assembly) = self
                .assembly
                .as_mut()
                .filter(|assembly| assembly.cid == cid)
            else {
                return Ok(());
            };
            if packet[4] != assembly.sequence {
                self.assembly = None;
                self.queue(cid, ERROR, &[ERR_INVALID_SEQ]);
                return Ok(());
            }
            let take = (assembly.length - assembly.data.len()).min(PACKET - 5);
            assembly.data.extend_from_slice(&packet[5..5 + take]);
            assembly.sequence += 1;
        }

        if self
            .assembly
            .as_ref()
            .is_some_and(|assembly| assembly.data.len() == assembly.length)
        {
            let assembly = self.assembly.take().expect("a finished assembly");
            if assembly.cid == BROADCAST && assembly.command != INIT {
                self.queue(BROADCAST, ERROR, &[ERR_INVALID_CHANNEL]);
            } else {
                self.dispatch(assembly.cid, assembly.command, assembly.data);
            }
        }
        Ok(())
    }

    fn receive(
        &mut self,
        wait: Duration,
        interrupt: Option<BorrowedFd<'_>>,
    ) -> io::Result<Option<Vec<u8>>> {
        if let Some(packet) = self.outbox.pop_front() {
            return Ok(Some(packet));
        }
        let awaiting_touch = self.deferred.is_some() || self.holding.is_some();
        let pause = match awaiting_touch {
            true => wait.min(KEEPALIVE_INTERVAL),
            false => wait,
        };
        match interrupt {
            Some(interrupt) => {
                let timeout = Timespec::try_from(pause).map_err(|_| io::ErrorKind::InvalidInput)?;
                let mut fds = [PollFd::new(&interrupt, PollFlags::IN)];
                match poll(&mut fds, Some(&timeout)) {
                    Ok(_) | Err(Errno::INTR) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            None => thread::sleep(pause),
        }
        if let Some((at, cid, _)) = &self.deferred {
            let cid = *cid;
            if Instant::now() < *at {
                self.queue(cid, KEEPALIVE, &[UP_NEEDED]);
            } else if let Some((_, _, response)) = self.deferred.take() {
                self.outbox.extend(response);
            }
            return Ok(self.outbox.pop_front());
        }
        if let Some(cid) = self.holding {
            self.queue(cid, KEEPALIVE, &[UP_NEEDED]);
            return Ok(self.outbox.pop_front());
        }
        Ok(None)
    }
}
