use std::os::fd::{AsFd as _, BorrowedFd};
use std::time::{Duration, Instant};

use crate::ceremony::Watch;
use crate::crypto;
use crate::error::{Error, Result};
use crate::hid::HidLink;

pub(crate) const MSG: u8 = 0x03;
pub(crate) const CBOR: u8 = 0x10;
const INIT: u8 = 0x06;
const WINK: u8 = 0x08;
const CANCEL: u8 = 0x11;
const KEEPALIVE: u8 = 0x3b;
const ERROR: u8 = 0x3f;

const BROADCAST: u32 = 0xffff_ffff;
const INIT_HEADER: usize = 7;
const CONTINUATION_HEADER: usize = 5;
const MAX_SEQUENCE: usize = 128;
const MIN_PACKET: usize = 16;

const CAPABILITY_WINK: u8 = 0x01;
const CAPABILITY_CBOR: u8 = 0x04;
const CAPABILITY_NMSG: u8 = 0x08;

const KEEPALIVE_UP_NEEDED: u8 = 0x02;
const ERROR_CHANNEL_BUSY: u8 = 0x06;

const CONTINUATION_WAIT: Duration = Duration::from_secs(1);
const INIT_WAIT: Duration = Duration::from_secs(1);
const INIT_ATTEMPTS: usize = 3;
const CANCEL_GRACE: Duration = Duration::from_millis(1500);
const BUSY_PAUSE: Duration = Duration::from_millis(100);
const DRAIN_LIMIT: usize = 64;

pub(crate) struct Channel {
    link: Box<dyn HidLink>,
    cid: u32,
    packet: usize,
    capabilities: u8,
}

enum Reply {
    Busy,
    Done(Vec<u8>),
}

impl Channel {
    pub(crate) fn open(link: Box<dyn HidLink>, watch: &Watch) -> Result<Channel> {
        let packet = link.packet_len();
        if packet < MIN_PACKET {
            return Err(Error::Protocol("HID report size too small for CTAPHID"));
        }

        let mut channel = Channel {
            link,
            cid: BROADCAST,
            packet,
            capabilities: 0,
        };
        channel.drain()?;

        for _ in 0..INIT_ATTEMPTS {
            watch.check()?;
            if let Some((cid, capabilities)) = channel.init()? {
                channel.cid = cid;
                channel.capabilities = capabilities;
                return Ok(channel);
            }
        }

        Err(Error::Timeout)
    }

    pub(crate) fn supports_cbor(&self) -> bool {
        self.capabilities & CAPABILITY_CBOR != 0
    }

    pub(crate) fn supports_msg(&self) -> bool {
        self.capabilities & CAPABILITY_NMSG == 0
    }

    pub(crate) fn supports_wink(&self) -> bool {
        self.capabilities & CAPABILITY_WINK != 0
    }

    pub(crate) fn wink(&mut self, watch: &Watch) -> Result<()> {
        self.transact(WINK, &[], watch).map(drop)
    }

    pub(crate) fn transact(
        &mut self,
        command: u8,
        payload: &[u8],
        watch: &Watch,
    ) -> Result<Vec<u8>> {
        let limit = self.packet - INIT_HEADER + MAX_SEQUENCE * (self.packet - CONTINUATION_HEADER);
        if payload.len() > limit || payload.len() > usize::from(u16::MAX) {
            return Err(Error::RequestTooLarge);
        }

        loop {
            watch.check()?;
            self.write(self.cid, command, payload)?;
            match self.reply(command, watch)? {
                Reply::Done(data) => return Ok(data),
                Reply::Busy => watch.pause(BUSY_PAUSE)?,
            }
        }
    }

    fn init(&mut self) -> Result<Option<(u32, u8)>> {
        let nonce: [u8; 8] = crypto::random()?;
        self.write(BROADCAST, INIT, &nonce)?;

        let until = Instant::now() + INIT_WAIT;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            let Some(packet) = self.receive(left, None)? else {
                continue;
            };
            if header(&packet) != Some((BROADCAST, INIT)) {
                continue;
            }
            let data = self.read_continuations(&packet)?;
            if data.len() < 17 || data[..8] != nonce {
                continue;
            }
            let cid = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
            if cid == BROADCAST || cid == 0 {
                return Err(Error::Protocol("device allocated a reserved channel"));
            }
            return Ok(Some((cid, data[16])));
        }
    }

    fn reply(&mut self, command: u8, watch: &Watch) -> Result<Reply> {
        let cancelled = watch.cancel_signal()?;
        let mut stopping: Option<(Error, Instant)> = None;

        loop {
            if stopping.is_none()
                && let Some(stop) = watch.stop_reason()
            {
                self.write(self.cid, CANCEL, &[])?;
                stopping = Some((stop, Instant::now() + CANCEL_GRACE));
            }

            let (wait, interrupt) = match &stopping {
                Some((_, until)) => (until.saturating_duration_since(Instant::now()), None),
                None => (watch.remaining(), Some(cancelled.as_fd())),
            };
            if wait.is_zero() {
                return Err(stopping.map_or(Error::Timeout, |(stop, _)| stop));
            }

            let Some(packet) = self.receive(wait, interrupt)? else {
                continue;
            };
            let Some((cid, found)) = header(&packet) else {
                continue;
            };
            if cid != self.cid {
                continue;
            }

            match found {
                KEEPALIVE => {
                    if packet.get(INIT_HEADER) == Some(&KEEPALIVE_UP_NEEDED) && stopping.is_none() {
                        watch.touch();
                    }
                }
                ERROR => {
                    let code = packet.get(INIT_HEADER).copied().unwrap_or_default();
                    if let Some((stop, _)) = stopping {
                        return Err(stop);
                    }
                    if code == ERROR_CHANNEL_BUSY {
                        return Ok(Reply::Busy);
                    }
                    return Err(Error::Hid(code));
                }
                found if found == command => {
                    let data = self.read_continuations(&packet)?;
                    if let Some((stop, _)) = stopping {
                        return Err(stop);
                    }
                    return Ok(Reply::Done(data));
                }
                _ => return Err(Error::Protocol("response to a different command")),
            }
        }
    }

    fn read_continuations(&mut self, first: &[u8]) -> Result<Vec<u8>> {
        let length = usize::from(u16::from_be_bytes([first[5], first[6]]));
        let mut data = Vec::with_capacity(length);
        let cid = u32::from_be_bytes([first[0], first[1], first[2], first[3]]);
        let head_end = first.len().min(INIT_HEADER + length);
        data.extend_from_slice(&first[INIT_HEADER..head_end]);

        let mut sequence = 0u8;
        while data.len() < length {
            let packet = self
                .receive(CONTINUATION_WAIT, None)?
                .ok_or(Error::Protocol("message truncated mid-transfer"))?;
            if packet.len() < CONTINUATION_HEADER
                || u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]) != cid
            {
                continue;
            }
            if packet[4] & 0x80 != 0 {
                return Err(Error::Protocol(
                    "new message started before previous one completed",
                ));
            }
            if packet[4] != sequence {
                return Err(Error::Protocol(
                    "a continuation packet arrived out of sequence",
                ));
            }
            let take = (length - data.len()).min(packet.len() - CONTINUATION_HEADER);
            data.extend_from_slice(&packet[CONTINUATION_HEADER..CONTINUATION_HEADER + take]);
            sequence = sequence.wrapping_add(1);
        }

        Ok(data)
    }

    fn write(&mut self, cid: u32, command: u8, payload: &[u8]) -> Result<()> {
        let mut packet = vec![0u8; self.packet];
        packet[..4].copy_from_slice(&cid.to_be_bytes());
        packet[4] = 0x80 | command;
        packet[5..7].copy_from_slice(&(payload.len() as u16).to_be_bytes());
        let first = payload.len().min(self.packet - INIT_HEADER);
        packet[INIT_HEADER..INIT_HEADER + first].copy_from_slice(&payload[..first]);
        self.link.send(&packet).map_err(Error::io)?;

        for (sequence, chunk) in payload[first..]
            .chunks(self.packet - CONTINUATION_HEADER)
            .enumerate()
        {
            let mut packet = vec![0u8; self.packet];
            packet[..4].copy_from_slice(&cid.to_be_bytes());
            packet[4] = sequence as u8;
            packet[CONTINUATION_HEADER..CONTINUATION_HEADER + chunk.len()].copy_from_slice(chunk);
            self.link.send(&packet).map_err(Error::io)?;
        }

        Ok(())
    }

    fn receive(
        &mut self,
        wait: Duration,
        interrupt: Option<BorrowedFd<'_>>,
    ) -> Result<Option<Vec<u8>>> {
        self.link.receive(wait, interrupt).map_err(Error::io)
    }

    fn drain(&mut self) -> Result<()> {
        for _ in 0..DRAIN_LIMIT {
            if self.receive(Duration::ZERO, None)?.is_none() {
                break;
            }
        }
        Ok(())
    }
}

fn header(packet: &[u8]) -> Option<(u32, u8)> {
    if packet.len() < INIT_HEADER || packet[4] & 0x80 == 0 {
        return None;
    }
    let cid = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
    Some((cid, packet[4] & 0x7f))
}
