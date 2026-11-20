use std::time::Instant;

use discord_core::VIDEO_CLOCK;

use crate::rtcp::{self, ReportBlock};

#[derive(Debug)]
pub(crate) struct Reception {
    origin: Instant,
    base: Option<u64>,
    highest: u64,
    received: u64,
    expected_before: u64,
    received_before: u64,
    transit: Option<i64>,
    jitter: f64,
    reported: Option<(u32, Instant)>,
}

impl Reception {
    pub(crate) fn new(origin: Instant) -> Reception {
        Reception {
            origin,
            base: None,
            highest: 0,
            received: 0,
            expected_before: 0,
            received_before: 0,
            transit: None,
            jitter: 0.0,
            reported: None,
        }
    }

    pub(crate) fn arrived(&mut self, sequence: u64, timestamp: u32, now: Instant) {
        let base = *self.base.get_or_insert(sequence);
        if sequence < base {
            self.base = Some(sequence);
        }
        self.highest = self.highest.max(sequence);
        self.received += 1;
        let arrival = now.saturating_duration_since(self.origin).as_micros() as i64
            * i64::from(VIDEO_CLOCK)
            / 1_000_000;
        let transit = arrival - i64::from(timestamp);
        if let Some(previous) = self.transit.replace(transit) {
            let difference = (transit - previous).unsigned_abs() as f64;
            self.jitter += (difference - self.jitter) / 16.0;
        }
    }

    pub(crate) fn sender_reported(&mut self, ntp: u64, now: Instant) {
        self.reported = Some((rtcp::middle(ntp), now));
    }

    pub(crate) fn block(&mut self, source: u32, now: Instant) -> Option<ReportBlock> {
        let base = self.base?;
        let expected = self.highest - base + 1;
        let lost = expected.saturating_sub(self.received);
        let expected_interval = expected - self.expected_before;
        let received_interval = self.received - self.received_before;
        self.expected_before = expected;
        self.received_before = self.received;
        let lost_interval = expected_interval.saturating_sub(received_interval);
        let fraction_lost = (lost_interval << 8)
            .checked_div(expected_interval)
            .unwrap_or(0)
            .min(255) as u8;
        let (last_report, since_report) = match self.reported {
            Some((middle, at)) => (
                middle,
                (now.saturating_duration_since(at).as_secs_f64() * 65_536.0) as u32,
            ),
            None => (0, 0),
        };
        Some(ReportBlock {
            source,
            fraction_lost,
            lost: lost.min(0x00FF_FFFF) as u32,
            highest: self.highest as u32,
            jitter: self.jitter as u32,
            last_report,
            since_report,
        })
    }
}
