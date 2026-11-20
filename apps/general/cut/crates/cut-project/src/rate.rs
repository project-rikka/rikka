use std::fmt;
use std::time::Duration;

use crate::error::{Error, Result};

const NANOS: i128 = 1_000_000_000;
const NTSC: u32 = 1001;
const NTSC_NOMINALS: [u32; 6] = [24, 30, 48, 60, 120, 240];
const INTEGER_SLACK: f64 = 0.01;
const NTSC_SLACK: f64 = 0.005;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rate {
    numerator: u32,
    denominator: u32,
}

impl Rate {
    pub const NTSC_FILM: Rate = Rate::ntsc(24);
    pub const FILM: Rate = Rate::whole(24);
    pub const PAL: Rate = Rate::whole(25);
    pub const NTSC_VIDEO: Rate = Rate::ntsc(30);
    pub const VIDEO: Rate = Rate::whole(30);
    pub const PAL_HIGH: Rate = Rate::whole(50);
    pub const NTSC_HIGH: Rate = Rate::ntsc(60);
    pub const HIGH: Rate = Rate::whole(60);
    pub const COMMON: [Rate; 8] = [
        Rate::NTSC_FILM,
        Rate::FILM,
        Rate::PAL,
        Rate::NTSC_VIDEO,
        Rate::VIDEO,
        Rate::PAL_HIGH,
        Rate::NTSC_HIGH,
        Rate::HIGH,
    ];

    const fn whole(frames: u32) -> Rate {
        Rate {
            numerator: frames,
            denominator: 1,
        }
    }

    const fn ntsc(nominal: u32) -> Rate {
        Rate {
            numerator: nominal * 1000,
            denominator: NTSC,
        }
    }

    pub fn new(numerator: u32, denominator: u32) -> Result<Rate> {
        if numerator == 0 || denominator == 0 {
            return Err(Error::Rate {
                numerator,
                denominator,
            });
        }
        let common = gcd(numerator, denominator);
        Ok(Rate {
            numerator: numerator / common,
            denominator: denominator / common,
        })
    }

    #[must_use]
    pub fn measured(per_second: f64) -> Option<Rate> {
        if !per_second.is_finite() || per_second <= 0.0 {
            return None;
        }
        let nearest = per_second.round();
        if (per_second - nearest).abs() < INTEGER_SLACK && nearest >= 1.0 {
            return Some(Rate::whole(nearest as u32));
        }
        if let Some(nominal) = NTSC_NOMINALS
            .into_iter()
            .find(|nominal| (per_second - Rate::ntsc(*nominal).per_second()).abs() < NTSC_SLACK)
        {
            return Some(Rate::ntsc(nominal));
        }
        Rate::new((per_second * 1000.0).round() as u32, 1000).ok()
    }

    #[must_use]
    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    #[must_use]
    pub fn per_second(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    #[must_use]
    pub fn nominal(self) -> u32 {
        self.per_second().round().max(1.0) as u32
    }

    #[must_use]
    pub(crate) fn drops_frames(self) -> bool {
        self.denominator == NTSC && self.numerator.is_multiple_of(30_000)
    }

    #[must_use]
    pub fn frames_in(self, duration: Duration) -> i64 {
        let nanos = duration.as_nanos() as i128;
        let frames = nanos * i128::from(self.numerator) / (NANOS * i128::from(self.denominator));
        frames.min(i128::from(i64::MAX)) as i64
    }

    #[must_use]
    pub fn start_of(self, frame: i64) -> Duration {
        self.nanos_at(i128::from(frame.max(0)) * 2)
    }

    #[must_use]
    pub fn time_at(self, frame: f64) -> Duration {
        Duration::from_secs_f64((frame / self.per_second()).max(0.0))
    }

    fn nanos_at(self, halves: i128) -> Duration {
        let span = 2 * i128::from(self.numerator);
        let nanos = (halves * NANOS * i128::from(self.denominator) + span - 1) / span;
        Duration::from_nanos(nanos.min(i128::from(u64::MAX)) as u64)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == 1 {
            return write!(f, "{}", self.numerator);
        }
        let formatted = format!("{:.3}", self.per_second());
        f.write_str(formatted.trim_end_matches('0').trim_end_matches('.'))
    }
}

pub(crate) fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
