use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::civil::Date;
use jiff::{Timestamp, Zoned};
use rikka_ui::Observers;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Period {
    Second,
    HalfMinute,
    #[default]
    Minute,
    FiveMinutes,
    HalfHour,
    Hour,
}

impl Period {
    const ALL: [Period; 6] = [
        Period::Second,
        Period::HalfMinute,
        Period::Minute,
        Period::FiveMinutes,
        Period::HalfHour,
        Period::Hour,
    ];

    const fn secs(self) -> u64 {
        match self {
            Period::Second => 1,
            Period::HalfMinute => 30,
            Period::Minute => 60,
            Period::FiveMinutes => 300,
            Period::HalfHour => 1800,
            Period::Hour => 3600,
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone)]
pub struct WallClock(Rc<Face>);

struct Face {
    last_tick: Cell<u64>,
    today: RefCell<Date>,
    periods: [Observers; Period::ALL.len()],
    day: Observers,
    needs: Cell<[usize; Period::ALL.len()]>,
    moved: Cell<bool>,
}

pub struct Need {
    wall: WallClock,
    period: Period,
}

impl WallClock {
    pub(super) fn new() -> WallClock {
        WallClock(Rc::new(Face {
            last_tick: Cell::new(unix().as_secs()),
            today: RefCell::new(Zoned::now().date()),
            periods: Default::default(),
            day: Observers::new(),
            needs: Cell::new([0; Period::ALL.len()]),
            moved: Cell::new(false),
        }))
    }

    #[must_use]
    pub fn need(&self, period: Period) -> Need {
        self.count(period, |count| count + 1);
        Need {
            wall: self.clone(),
            period,
        }
    }

    pub fn follow(&self, period: Period) {
        self.0.periods[period.index()].read();
    }

    pub fn painting(&self, period: Period) {
        self.0.periods[period.index()].painting();
    }

    #[must_use]
    pub fn now(&self, period: Period) -> Timestamp {
        self.follow(period);
        Timestamp::now()
    }

    #[must_use]
    pub fn slot(&self, period: Period) -> u64 {
        self.follow(period);
        unix().as_secs() / period.secs()
    }

    #[must_use]
    pub fn today(&self) -> Date {
        self.0.day.read();
        Zoned::now().date()
    }

    pub fn advance(&self) {
        let now = unix().as_secs();
        let before = self.0.last_tick.replace(now);
        for (period, observers) in Period::ALL.into_iter().zip(&self.0.periods) {
            if now / period.secs() != before / period.secs() {
                observers.changed();
            }
        }
        let date = Zoned::now().date();
        if self.0.today.replace(date) != date {
            self.0.day.changed();
        }
    }

    pub fn take_moved(&self) -> bool {
        self.0.moved.replace(false)
    }

    #[must_use]
    pub fn next(&self) -> Option<SystemTime> {
        let needs = self.0.needs.get();
        let finest = Period::ALL
            .into_iter()
            .zip(needs)
            .find(|(_, count)| *count > 0)
            .map(|(period, _)| period.secs())?;
        let now = unix().as_secs();
        let boundary = UNIX_EPOCH + Duration::from_secs((now / finest + 1) * finest);
        let midnight = Zoned::now()
            .tomorrow()
            .and_then(|tomorrow| tomorrow.start_of_day())
            .map(|start| SystemTime::from(start.timestamp()))
            .ok();
        Some(midnight.map_or(boundary, |midnight| boundary.min(midnight)))
    }

    fn count(&self, period: Period, change: impl FnOnce(usize) -> usize) {
        let mut needs = self.0.needs.get();
        needs[period.index()] = change(needs[period.index()]);
        self.0.needs.set(needs);
        self.0.moved.set(true);
    }
}

impl Drop for Need {
    fn drop(&mut self) {
        self.wall
            .count(self.period, |count| count.saturating_sub(1));
    }
}

fn unix() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

impl std::fmt::Debug for WallClock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WallClock")
            .field("needs", &self.0.needs.get())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Need {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Need").field(&self.period).finish()
    }
}
