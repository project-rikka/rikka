use std::collections::BTreeMap;
use std::time::Instant;

use rikka_rate_control::Acknowledgement;
use rikka_remote_wire::{Arrival, Feedback};

const LOST_REPEATS: u8 = 3;
const MOST_LOST: usize = 32;
const LONGEST_RANGE: u32 = 1 << 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameReport {
    pub stream: u16,
    pub good: Option<u32>,
}

#[derive(Debug)]
pub struct FeedbackBuilder {
    epoch: Instant,
    arrivals: BTreeMap<u32, u32>,
    next_base: Option<u32>,
    highest: Option<u32>,
    lost: Vec<(u16, u32, u8)>,
}

impl FeedbackBuilder {
    #[must_use]
    pub fn new(epoch: Instant) -> FeedbackBuilder {
        FeedbackBuilder {
            epoch,
            arrivals: BTreeMap::new(),
            next_base: None,
            highest: None,
            lost: Vec::new(),
        }
    }

    pub fn received(&mut self, seq: u32, at: Instant) {
        let base = *self.next_base.get_or_insert(seq);
        if seq < base {
            return;
        }
        let at_us = at.saturating_duration_since(self.epoch).as_micros() as u32;
        self.arrivals.entry(seq).or_insert(at_us);
        self.highest = Some(self.highest.map_or(seq, |highest| highest.max(seq)));
    }

    pub fn lost_frame(&mut self, stream: u16, frame: u32) {
        if self
            .lost
            .iter()
            .any(|(known_stream, known, _)| *known_stream == stream && *known == frame)
        {
            return;
        }
        self.lost.push((stream, frame, LOST_REPEATS));
        if self.lost.len() > MOST_LOST {
            self.lost.remove(0);
        }
    }

    pub fn build(&mut self, report: FrameReport, budget: usize) -> Vec<Feedback> {
        self.lost.retain(|(stream, _, _)| *stream == report.stream);
        let lost: Vec<u32> = self.lost.iter().map(|(_, frame, _)| *frame).collect();
        for (_, _, repeats) in &mut self.lost {
            *repeats -= 1;
        }
        self.lost.retain(|(_, _, repeats)| *repeats > 0);
        let mut feedbacks = Vec::new();
        while let (Some(base), Some(highest)) = (self.next_base, self.highest)
            && highest >= base
        {
            let wanted = (highest - base + 1).min(LONGEST_RANGE);
            let count = fitting(&self.arrivals, base, wanted, lost.len(), budget);
            if count == 0 {
                break;
            }
            let end = base + count;
            let arrivals: Vec<Arrival> = self
                .arrivals
                .range(base..end)
                .map(|(&seq, &at_us)| Arrival { seq, at_us })
                .collect();
            self.arrivals = self.arrivals.split_off(&end);
            self.next_base = Some(end);
            feedbacks.push(Feedback {
                base_seq: base,
                count: count as u16,
                arrivals,
                stream: report.stream,
                good: report.good,
                lost: lost.clone(),
            });
        }
        if feedbacks.is_empty() {
            feedbacks.push(Feedback {
                base_seq: self.next_base.unwrap_or(0),
                count: 0,
                arrivals: Vec::new(),
                stream: report.stream,
                good: report.good,
                lost,
            });
        }
        feedbacks
    }
}

pub fn acknowledgement(
    feedback: &Feedback,
) -> Acknowledgement<impl Iterator<Item = rikka_rate_control::Arrival> + '_> {
    Acknowledgement {
        base_seq: feedback.base_seq,
        count: feedback.count,
        arrivals: feedback
            .arrivals
            .iter()
            .map(|arrival| rikka_rate_control::Arrival {
                seq: arrival.seq,
                at_us: arrival.at_us,
            }),
    }
}

fn fitting(
    arrivals: &BTreeMap<u32, u32>,
    base: u32,
    wanted: u32,
    lost: usize,
    budget: usize,
) -> u32 {
    let mut count = wanted.min(u32::from(u16::MAX));
    loop {
        let received = arrivals.range(base..base + count).count();
        if Feedback::bytes_for(count as usize, received, lost) <= budget || count <= 1 {
            return count;
        }
        count /= 2;
    }
}
