use std::collections::VecDeque;
use std::time::Duration;

use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::Pacing;

use super::frame_outcome::FrameOutcome;
use super::history::History;
use super::lateness::Lateness;
use crate::timing::{Timing, vblank_from};

const MARGIN: Duration = Duration::from_micros(1_500);

const PENALTY_STEP: u32 = 4;

const PENALTY_DECAY: u32 = 256;

const AHEAD_HYSTERESIS: (u32, u32) = (2, 3);

const MISSES_AHEAD: u32 = 2;

const MAX_FRAMES_IN_FLIGHT: usize = 2;

#[derive(Debug, Clone, Copy)]
struct FrameTarget {
    presentation: Duration,
    predicted: bool,
    late: bool,
}

#[derive(Debug)]
pub struct FramePacer {
    history: History,
    penalty: Duration,
    lead: Duration,
    planned: Option<FrameTarget>,
    drawing: Option<FrameTarget>,
    in_flight: VecDeque<FrameTarget>,
    started: Duration,
    refresh: Duration,
    ahead: bool,
    pacing: Pacing,
}

impl FramePacer {
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            history: History::new(output.into()),
            penalty: Duration::ZERO,
            lead: Duration::ZERO,
            planned: None,
            drawing: None,
            in_flight: VecDeque::with_capacity(MAX_FRAMES_IN_FLIGHT),
            started: now(),
            refresh: Duration::ZERO,
            ahead: false,
            pacing: Pacing::Balanced,
        }
    }

    pub fn frames_in_flight(&self) -> usize {
        match self.ahead {
            true => MAX_FRAMES_IN_FLIGHT,
            false => 1,
        }
    }

    pub fn set_pacing(&mut self, pacing: Pacing) {
        self.pacing = pacing;
    }

    pub fn plan(&mut self, timing: &Timing, not_before: Duration) -> Duration {
        self.lead = timing.lead;
        let now = now();
        match timing.vrr || self.pacing == Pacing::Immediate {
            true => {
                self.ahead = false;
                self.schedule_frame(
                    now,
                    None,
                    timing.refresh,
                    not_before,
                    self.budget(timing.refresh),
                )
            }
            false => {
                let phase = timing
                    .phase(Timestamp::from_monotonic(now))
                    .map(Timestamp::as_monotonic);
                self.plan_fixed_rate(now, phase, timing.refresh, not_before)
            }
        }
    }

    pub fn begin(&mut self, refresh: Duration, lateness: Lateness) -> Timestamp {
        let now = now();
        self.started = now;
        self.refresh = refresh;
        let mut target = self.planned.unwrap_or(FrameTarget {
            presentation: now + refresh,
            predicted: false,
            late: false,
        });
        target.late = self.history.started(lateness);
        self.history.drawing_ahead(self.ahead);
        self.drawing = Some(target);
        Timestamp::from_monotonic(target.presentation)
    }

    pub fn finished(&mut self, turn: FrameOutcome) {
        let work = now().saturating_sub(self.started);
        self.history.finished(work, self.absorbable(), turn);
        if let (FrameOutcome::Committed, Some(target)) = (turn, self.drawing.take()) {
            if self.in_flight.len() == MAX_FRAMES_IN_FLIGHT {
                self.in_flight.pop_front();
            }
            self.in_flight.push_back(target);
        }
    }

    pub fn presented(&mut self, at: Timestamp, refresh: Duration) {
        let Some(target) = self.in_flight.pop_front().filter(|target| target.predicted) else {
            return;
        };
        let missed = at.as_monotonic() > target.presentation + refresh / 2;
        self.history.presented(missed);
        match (missed, target.late) {
            (true, false) => self.penalty = (self.penalty + refresh / PENALTY_STEP).min(refresh),
            (true, true) => {}
            (false, _) => self.penalty = self.penalty.saturating_sub(refresh / PENALTY_DECAY),
        }
    }

    pub fn discarded(&mut self) {
        self.in_flight.pop_front();
    }

    pub fn rendered(&mut self, after: Duration, done: Timestamp, aimed: Timestamp) {
        let late = done + self.lead > aimed;
        self.history.rendered(after, self.absorbable(), late);
    }

    fn absorbable(&self) -> Duration {
        match self.refresh.is_zero() {
            true => Duration::MAX,
            false => self.refresh * MAX_FRAMES_IN_FLIGHT as u32,
        }
    }

    pub fn stalled(&mut self, waited: Duration) {
        self.history.stalled(waited);
    }

    pub fn scanned_out(&mut self, direct: bool) {
        self.history.scanned_out(direct);
    }

    pub fn reset(&mut self) {
        self.planned = None;
        self.drawing = None;
        self.in_flight.clear();
    }

    fn plan_fixed_rate(
        &mut self,
        now: Duration,
        phase: Option<Duration>,
        refresh: Duration,
        not_before: Duration,
    ) -> Duration {
        let desired = self.desired_budget(refresh);
        self.update_ahead(refresh, desired);
        self.schedule_frame(
            now,
            phase,
            refresh,
            not_before,
            desired.min(self.ceiling(refresh)),
        )
    }

    fn schedule_frame(
        &mut self,
        now: Duration,
        phase: Option<Duration>,
        refresh: Duration,
        not_before: Duration,
        budget: Duration,
    ) -> Duration {
        let target = self.target_for(now + not_before + budget, phase, refresh);
        let delay = target.presentation.saturating_sub(now + budget);
        self.planned = Some(target);
        delay
    }

    fn target_for(
        &self,
        earliest: Duration,
        phase: Option<Duration>,
        refresh: Duration,
    ) -> FrameTarget {
        let next_free = self
            .planned
            .map_or(Duration::ZERO, |target| target.presentation + refresh);
        let (presentation, predicted) = match phase {
            Some(vblank) => (
                vblank_from(vblank, refresh, earliest).unwrap_or(vblank),
                true,
            ),
            None => (earliest, false),
        };
        FrameTarget {
            presentation: presentation.max(next_free),
            predicted,
            late: false,
        }
    }

    fn update_ahead(&mut self, refresh: Duration, desired: Duration) {
        match self.pacing {
            Pacing::LowLatency | Pacing::Immediate => {
                self.ahead = false;
                return;
            }
            Pacing::Steady => {
                self.ahead = true;
                return;
            }
            Pacing::Balanced => {}
        }
        if self.history.missed_recently() >= MISSES_AHEAD {
            self.ahead = true;
            return;
        }
        let (numerator, denominator) = AHEAD_HYSTERESIS;
        self.ahead = match self.ahead {
            false => desired >= refresh,
            true => desired * denominator >= refresh * numerator,
        };
    }

    fn desired_budget(&self, refresh: Duration) -> Duration {
        self.history.work(refresh) + self.history.lateness() + MARGIN + self.penalty + self.lead
    }

    fn ceiling(&self, refresh: Duration) -> Duration {
        match self.ahead {
            true => refresh * 2,
            false => refresh,
        }
    }

    fn budget(&self, refresh: Duration) -> Duration {
        self.desired_budget(refresh).min(self.ceiling(refresh))
    }
}

fn now() -> Duration {
    Timestamp::now().as_monotonic()
}
