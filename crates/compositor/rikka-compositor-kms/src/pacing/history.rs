use std::time::{Duration, Instant};

use rikka_compositor_reactor::Timestamp;

use super::cost_estimate::CostEstimate;
use super::duration_stats::DurationStats;
use super::frame_outcome::FrameOutcome;
use super::lateness::Lateness;
use super::peak::DecayingPeak;

const UNTRIED_COST: u32 = 2;

const MISS_WINDOW: u32 = 32;
const _: () = assert!(MISS_WINDOW == u32::BITS);

const REPORT_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(crate) struct History {
    committed: CostEstimate,
    rendered: CostEstimate,
    empty: CostEstimate,
    lateness: DecayingPeak,
    misses: u32,
    report: Report,
}

impl History {
    pub(crate) fn new(output: String) -> Self {
        Self {
            committed: CostEstimate::default(),
            rendered: CostEstimate::default(),
            empty: CostEstimate::default(),
            lateness: DecayingPeak::default(),
            misses: 0,
            report: Report::new(output),
        }
    }

    pub(crate) fn started(&mut self, lateness: Lateness) -> bool {
        let late = lateness.total();
        let unbudgeted = late > self.lateness.get();
        self.lateness.add(late);
        self.report.wake.add(lateness.timer_delay());
        self.report.dispatch.add(lateness.dispatched());
        unbudgeted
    }

    pub(crate) fn finished(&mut self, work: Duration, absorbable: Duration, turn: FrameOutcome) {
        let now = Timestamp::now();
        let budgeted = work.min(absorbable);
        match turn {
            FrameOutcome::Committed => self.committed.add(budgeted, now),
            FrameOutcome::Empty => {
                self.empty.add(budgeted, now);
                self.report.empty += 1;
            }
        }
        self.report.work.add(work);
    }

    pub(crate) fn presented(&mut self, missed: bool) {
        self.misses = (self.misses << 1) | u32::from(missed);
        self.report.presented += 1;
        self.report.missed += u64::from(missed);
        self.report.run = match missed {
            true => self.report.run + 1,
            false => 0,
        };
        self.report.worst_run = self.report.worst_run.max(self.report.run);
        self.report.log_if_due();
    }

    pub(crate) fn stalled(&mut self, waited: Duration) {
        self.report.stall.add(waited);
    }

    pub(crate) fn rendered(&mut self, ready: Duration, absorbable: Duration, late: bool) {
        self.rendered.add(ready.min(absorbable), Timestamp::now());
        self.report.ready.add(ready);
        self.report.rendered_late += u64::from(late);
    }

    pub(crate) fn drawing_ahead(&mut self, ahead: bool) {
        self.report.ahead += u64::from(ahead);
    }

    pub(crate) fn scanned_out(&mut self, direct: bool) {
        self.report.scanout += u64::from(direct);
    }

    pub(crate) fn work(&self, refresh: Duration) -> Duration {
        let untried = refresh / UNTRIED_COST;
        let drawing = self.committed.estimate().unwrap_or(untried);
        drawing
            .max(self.rendered.estimate().unwrap_or_default())
            .max(self.empty.estimate().unwrap_or_default())
    }

    pub(crate) fn lateness(&self) -> Duration {
        self.lateness.get()
    }

    pub(crate) fn missed_recently(&self) -> u32 {
        self.misses.count_ones()
    }
}

#[derive(Debug)]
struct Report {
    output: String,
    since: Instant,
    wake: DurationStats,
    dispatch: DurationStats,
    work: DurationStats,
    stall: DurationStats,
    ready: DurationStats,
    ahead: u64,
    scanout: u64,
    empty: u64,
    presented: u64,
    missed: u64,
    rendered_late: u64,
    run: u64,
    worst_run: u64,
}

impl Report {
    fn new(output: String) -> Self {
        Self {
            output,
            since: Instant::now(),
            wake: DurationStats::default(),
            dispatch: DurationStats::default(),
            work: DurationStats::default(),
            stall: DurationStats::default(),
            ready: DurationStats::default(),
            ahead: 0,
            scanout: 0,
            empty: 0,
            presented: 0,
            missed: 0,
            rendered_late: 0,
            run: 0,
            worst_run: 0,
        }
    }

    fn log_if_due(&mut self) {
        if self.since.elapsed() < REPORT_EVERY {
            return;
        }
        if self.work.count > 0 {
            tracing::debug!(
                output = self.output,
                frames = self.work.count,
                missed = self.missed,
                rendered_late = self.rendered_late,
                of = self.presented,
                worst_run = self.worst_run,
                ahead = self.ahead,
                scanout = self.scanout,
                empty = self.empty,
                wake = ?self.wake.mean(),
                wake_worst = ?self.wake.longest,
                dispatch = ?self.dispatch.mean(),
                dispatch_worst = ?self.dispatch.longest,
                work = ?self.work.mean(),
                work_worst = ?self.work.longest,
                ready = ?self.ready.mean(),
                ready_worst = ?self.ready.longest,
                stalls = self.stall.count,
                stalled = ?self.stall.total,
                "frame timeline"
            );
        }
        *self = Self {
            run: self.run,
            ..Self::new(std::mem::take(&mut self.output))
        };
    }
}
