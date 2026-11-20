use std::time::{Duration, Instant};

const GAIN: f64 = 0.125;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameClock {
    shown: Option<Instant>,
    ahead: Option<Instant>,
    vsync: Option<Vsync>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vsync {
    at: Instant,
    refresh: Duration,
}

impl Vsync {
    fn first_after(self, moment: Instant) -> Instant {
        let refresh = self.refresh.as_nanos();
        let span = |refreshes: u128| {
            Duration::from_nanos(u64::try_from(refreshes * refresh).unwrap_or(u64::MAX))
        };
        match moment.checked_duration_since(self.at) {
            Some(since) => self.at + span(since.as_nanos().div_ceil(refresh)),
            None => self.at - span(self.at.duration_since(moment).as_nanos() / refresh),
        }
    }
}

impl FrameClock {
    #[must_use]
    pub fn new() -> FrameClock {
        FrameClock::default()
    }

    pub fn presented(&mut self, at: Instant, refresh: Option<Duration>) {
        self.vsync = refresh
            .filter(|refresh| !refresh.is_zero())
            .map(|refresh| Vsync { at, refresh });
    }

    #[must_use]
    pub fn refresh(&self) -> Option<Duration> {
        self.vsync.map(|vsync| vsync.refresh)
    }

    pub fn presenting(&mut self, now: Instant, period: Duration) -> Instant {
        let next = match self.vsync {
            Some(vsync) => self.on_vsync(vsync, vsync.first_after(now + vsync.refresh / 2)),
            None => self.predicted(now, period),
        };
        let next = self.ahead.take().map_or(next, |ahead| next.max(ahead));
        self.shown = Some(next);
        next
    }

    pub fn ahead(&mut self, now: Instant, period: Duration) -> Instant {
        let period = self.vsync.map_or(period, |vsync| vsync.refresh);
        let next = match self.shown {
            Some(last) if last + period >= now => last + period,
            _ => match self.vsync {
                Some(vsync) => vsync.first_after(now + period / 2),
                None => now + period,
            },
        };
        self.ahead = Some(next);
        next
    }

    pub fn commit_ahead(&mut self) {
        if let Some(ahead) = self.ahead.take() {
            self.shown = Some(ahead);
        }
    }

    fn on_vsync(&self, vsync: Vsync, next: Instant) -> Instant {
        match self.shown {
            Some(last) if next <= last => vsync.first_after(last + vsync.refresh / 2),
            _ => next,
        }
    }

    fn predicted(&self, now: Instant, period: Duration) -> Instant {
        let ideal = now + period;
        let Some(last) = self.shown else {
            return ideal;
        };
        let predicted = last + period;
        let error = signed(ideal, predicted);
        match error.abs() > period.as_secs_f64() / 2.0 {
            true => ideal,
            false => shifted(predicted, error * GAIN),
        }
        .max(last)
    }
}

fn signed(later: Instant, earlier: Instant) -> f64 {
    match later >= earlier {
        true => later.duration_since(earlier).as_secs_f64(),
        false => -earlier.duration_since(later).as_secs_f64(),
    }
}

fn shifted(at: Instant, by: f64) -> Instant {
    let span = Duration::from_secs_f64(by.abs());
    match by >= 0.0 {
        true => at + span,
        false => at - span,
    }
}
