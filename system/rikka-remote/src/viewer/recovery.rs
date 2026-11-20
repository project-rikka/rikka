use std::collections::VecDeque;
use std::time::{Duration, Instant};

use rikka_remote_wire::Feedback;

use crate::media::Recover;

const SHORTEST_STALL: Duration = Duration::from_millis(300);
const STALL_RTTS: u32 = 4;
const REMEMBERED: usize = 1024;

#[derive(Debug, Default)]
pub(crate) struct Recovery {
    stream: Option<u16>,
    unconfirmed: VecDeque<(u32, Instant)>,
    abandoned: VecDeque<u32>,
    good: Option<u32>,
    handled_through: Option<u32>,
}

impl Recovery {
    pub(crate) fn begin_stream(&mut self, stream: u16) {
        *self = Recovery {
            stream: Some(stream),
            ..Recovery::default()
        };
    }

    pub(crate) fn sent(&mut self, frame: u32, at: Instant) {
        self.unconfirmed.push_back((frame, at));
        if self.unconfirmed.len() > REMEMBERED {
            self.unconfirmed.pop_front();
        }
    }

    pub(crate) fn abandoned(&mut self, frame: u32) {
        self.unconfirmed.retain(|(sent, _)| *sent != frame);
        self.abandoned.push_back(frame);
        if self.abandoned.len() > REMEMBERED {
            self.abandoned.pop_front();
        }
    }

    pub(crate) fn feedback(&mut self, feedback: &Feedback) -> Option<Recover> {
        if self.stream != Some(feedback.stream) {
            return None;
        }
        if let Some(good) = feedback.good
            && self.good.is_none_or(|known| good > known)
        {
            self.good = Some(good);
            self.unconfirmed.retain(|(sent, _)| *sent > good);
        }
        let fresh = feedback.lost.iter().any(|lost| {
            !self.abandoned.contains(lost)
                && self.handled_through.is_none_or(|handled| *lost > handled)
        });
        fresh.then(|| self.recover())
    }

    pub(crate) fn stalled(&mut self, now: Instant, rtt: Duration) -> Option<Recover> {
        let patience = SHORTEST_STALL.max(rtt * STALL_RTTS);
        let (frame, sent) = *self
            .unconfirmed
            .iter()
            .find(|(frame, _)| self.handled_through.is_none_or(|handled| *frame > handled))?;
        (now.saturating_duration_since(sent) > patience
            && self.good.is_none_or(|good| good < frame))
        .then(|| self.recover())
    }

    pub(crate) fn next_check(&self, rtt: Duration) -> Option<Instant> {
        let patience = SHORTEST_STALL.max(rtt * STALL_RTTS);
        self.unconfirmed
            .iter()
            .find(|(frame, _)| self.handled_through.is_none_or(|handled| *frame > handled))
            .map(|(_, sent)| *sent + patience)
    }

    fn recover(&mut self) -> Recover {
        self.handled_through = self
            .unconfirmed
            .back()
            .map(|(frame, _)| *frame)
            .or(self.handled_through);
        self.good.map_or(Recover::Intra, Recover::From)
    }
}
