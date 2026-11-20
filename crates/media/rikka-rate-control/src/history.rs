use std::collections::VecDeque;
use std::time::{Duration, Instant};

const KEPT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sent {
    pub seq: u32,
    pub at: Instant,
    pub bytes: usize,
    pub probe: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrival {
    pub seq: u32,
    pub at_us: u32,
}

#[derive(Clone, Debug)]
pub struct Acknowledgement<I> {
    pub base_seq: u32,
    pub count: u16,
    pub arrivals: I,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketResult {
    pub sent: Sent,
    pub arrival_us: Option<i64>,
}

#[derive(Debug, Default)]
pub struct History {
    packets: VecDeque<Sent>,
    clock: Option<(u32, i64)>,
}

impl History {
    #[must_use]
    pub fn new() -> History {
        History::default()
    }

    pub fn sent(&mut self, packet: Sent) {
        while self
            .packets
            .front()
            .is_some_and(|oldest| packet.at.saturating_duration_since(oldest.at) > KEPT)
        {
            self.packets.pop_front();
        }
        self.packets.push_back(packet);
    }

    pub fn acknowledge<I>(&mut self, acknowledgement: Acknowledgement<I>) -> Vec<PacketResult>
    where
        I: IntoIterator<Item = Arrival>,
    {
        let Some(first) = self.packets.front().map(|packet| packet.seq) else {
            return Vec::new();
        };
        let mut arrivals = acknowledgement.arrivals.into_iter().peekable();
        let end = acknowledgement
            .base_seq
            .saturating_add(u32::from(acknowledgement.count));
        let mut results = Vec::new();
        for seq in acknowledgement.base_seq.max(first)..end {
            let Some(index) = seq.checked_sub(first).map(|offset| offset as usize) else {
                continue;
            };
            let Some(sent) = self.packets.get(index).copied() else {
                break;
            };
            if sent.seq != seq {
                continue;
            }
            while arrivals.peek().is_some_and(|arrival| arrival.seq < seq) {
                arrivals.next();
            }
            let arrival_us = match arrivals.peek() {
                Some(arrival) if arrival.seq == seq => {
                    let at = arrival.at_us;
                    arrivals.next();
                    Some(self.unwrap(at))
                }
                _ => None,
            };
            results.push(PacketResult { sent, arrival_us });
        }
        results
    }

    fn unwrap(&mut self, at_us: u32) -> i64 {
        let unwrapped = match self.clock {
            Some((last, value)) => value + i64::from(at_us.wrapping_sub(last) as i32),
            None => i64::from(at_us),
        };
        self.clock = Some((at_us, unwrapped));
        unwrapped
    }
}
