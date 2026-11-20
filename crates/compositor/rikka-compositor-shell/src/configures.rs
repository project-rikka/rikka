use std::collections::VecDeque;

pub(crate) const UNACKED_LIMIT: usize = 64;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AckResult<C> {
    Configure(C),
    Dropped,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
struct SerialRange {
    first: u32,
    last: u32,
}

impl SerialRange {
    fn contains(self, serial: u32) -> bool {
        serial.wrapping_sub(self.first) <= self.last.wrapping_sub(self.first)
    }

    fn after(self, serial: u32) -> Option<SerialRange> {
        (serial != self.last).then(|| SerialRange {
            first: serial.wrapping_add(1),
            last: self.last,
        })
    }
}

#[derive(Debug)]
pub(crate) struct Configures<C> {
    sent: VecDeque<(u32, C)>,
    dropped: Option<SerialRange>,
    stale: Option<SerialRange>,
}

impl<C> Configures<C> {
    pub(crate) fn push(&mut self, serial: u32, configure: C) {
        if self.sent.len() == UNACKED_LIMIT
            && let Some((oldest, _)) = self.sent.pop_front()
        {
            let first = self.dropped.map_or(oldest, |dropped| dropped.first);
            self.dropped = Some(SerialRange {
                first,
                last: oldest,
            });
        }
        self.sent.push_back((serial, configure));
    }

    #[must_use]
    pub(crate) fn into_stale(self) -> Self {
        let first = self
            .stale
            .or(self.dropped)
            .map(|range| range.first)
            .or_else(|| self.sent.front().map(|(serial, _)| *serial));
        let last = self
            .sent
            .back()
            .map(|(serial, _)| *serial)
            .or_else(|| self.dropped.map(|range| range.last))
            .or_else(|| self.stale.map(|range| range.last));
        Self {
            sent: VecDeque::new(),
            dropped: None,
            stale: first
                .zip(last)
                .map(|(first, last)| SerialRange { first, last }),
        }
    }

    pub(crate) fn ack(&mut self, serial: u32) -> AckResult<C> {
        if let Some(index) = self.sent.iter().position(|(sent, _)| *sent == serial)
            && let Some((_, configure)) = self.sent.drain(..=index).last()
        {
            self.dropped = None;
            self.stale = None;
            return AckResult::Configure(configure);
        }
        if let Some(dropped) = self.dropped.filter(|dropped| dropped.contains(serial)) {
            self.dropped = dropped.after(serial);
            return AckResult::Dropped;
        }
        if let Some(stale) = self.stale.filter(|stale| stale.contains(serial)) {
            self.stale = stale.after(serial);
            return AckResult::Stale;
        }
        AckResult::Unknown
    }

    pub(crate) fn last(&self) -> Option<&C> {
        self.sent.back().map(|(_, configure)| configure)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sent.is_empty()
    }
}

impl<C> Default for Configures<C> {
    fn default() -> Self {
        Self {
            sent: VecDeque::new(),
            dropped: None,
            stale: None,
        }
    }
}
