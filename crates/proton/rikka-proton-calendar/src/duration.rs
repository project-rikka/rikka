use std::fmt;
use std::str::FromStr;

use crate::civil::DAY;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Duration {
    pub negative: bool,
    pub weeks: u32,
    pub days: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
}

impl Duration {
    #[must_use]
    pub fn from_seconds(total: i64) -> Duration {
        let negative = total < 0;
        let mut rest = total.unsigned_abs();
        let days = rest / DAY.unsigned_abs();
        rest %= DAY.unsigned_abs();
        let mut duration = Duration {
            negative,
            weeks: 0,
            days: days as u32,
            hours: (rest / 3600) as u32,
            minutes: (rest % 3600 / 60) as u32,
            seconds: (rest % 60) as u32,
        };
        if duration.days.is_multiple_of(7)
            && duration.hours == 0
            && duration.minutes == 0
            && duration.seconds == 0
        {
            duration.weeks = duration.days / 7;
            duration.days = 0;
        }
        duration
    }

    #[must_use]
    pub fn minutes(minutes: i64) -> Duration {
        Duration::from_seconds(minutes * 60)
    }

    #[must_use]
    pub fn total_seconds(&self) -> i64 {
        let magnitude = i64::from(self.weeks) * 7 * DAY
            + i64::from(self.days) * DAY
            + i64::from(self.hours) * 3600
            + i64::from(self.minutes) * 60
            + i64::from(self.seconds);
        if self.negative { -magnitude } else { magnitude }
    }

    #[must_use]
    pub fn nominal_days(&self) -> i64 {
        let days = i64::from(self.weeks) * 7 + i64::from(self.days);
        if self.negative { -days } else { days }
    }

    #[must_use]
    pub fn exact_seconds(&self) -> i64 {
        let seconds =
            i64::from(self.hours) * 3600 + i64::from(self.minutes) * 60 + i64::from(self.seconds);
        if self.negative { -seconds } else { seconds }
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.total_seconds() == 0
    }
}

impl FromStr for Duration {
    type Err = Error;

    fn from_str(text: &str) -> Result<Duration> {
        let invalid = || Error::InvalidValue("DURATION".to_owned(), text.to_owned());
        let mut rest = text.trim();
        let mut duration = Duration::default();
        if let Some(after) = rest.strip_prefix('-') {
            duration.negative = true;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('+') {
            rest = after;
        }
        let rest = rest.strip_prefix(['P', 'p']).ok_or_else(invalid)?;
        let mut in_time = false;
        let mut number = String::new();
        let mut has_digits = false;

        for character in rest.chars() {
            match character.to_ascii_uppercase() {
                'T' => {
                    if in_time || !number.is_empty() {
                        return Err(invalid());
                    }
                    in_time = true;
                }
                digit if digit.is_ascii_digit() => number.push(digit),
                unit => {
                    let value: u32 = number.parse().map_err(|_| invalid())?;
                    number.clear();
                    has_digits = true;
                    match (unit, in_time) {
                        ('W', false) => duration.weeks = value,
                        ('D', false) => duration.days = value,
                        ('H', true) => duration.hours = value,
                        ('M', true) => duration.minutes = value,
                        ('S', true) => duration.seconds = value,
                        _ => return Err(invalid()),
                    }
                }
            }
        }

        if !number.is_empty() || !has_digits {
            return Err(invalid());
        }
        Ok(duration)
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative && !self.is_zero() {
            formatter.write_str("-")?;
        }
        formatter.write_str("P")?;
        if self.weeks > 0 {
            write!(formatter, "{}W", self.weeks)?;
        }
        if self.days > 0 {
            write!(formatter, "{}D", self.days)?;
        }
        if self.hours > 0 || self.minutes > 0 || self.seconds > 0 {
            formatter.write_str("T")?;
            if self.hours > 0 {
                write!(formatter, "{}H", self.hours)?;
            }
            if self.minutes > 0 {
                write!(formatter, "{}M", self.minutes)?;
            }
            if self.seconds > 0 {
                write!(formatter, "{}S", self.seconds)?;
            }
        }
        if self.is_zero() {
            formatter.write_str("T0S")?;
        }
        Ok(())
    }
}
