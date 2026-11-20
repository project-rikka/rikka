use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

use crate::alarm::{Alarm, AlarmAction, Trigger};
use crate::civil::DAY;
use crate::date_value::DateValue;
use crate::duration::Duration;
use crate::error::Result;
use crate::series::Instance;
use crate::zones::Zones;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum NotificationKind {
    Email,
    Device,
}

impl From<i32> for NotificationKind {
    fn from(value: i32) -> NotificationKind {
        match value {
            0 => NotificationKind::Email,
            _ => NotificationKind::Device,
        }
    }
}

impl From<NotificationKind> for i32 {
    fn from(kind: NotificationKind) -> i32 {
        match kind {
            NotificationKind::Email => 0,
            NotificationKind::Device => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Notification {
    #[serde(rename = "Type")]
    kind: NotificationKind,
    #[serde(rename = "Trigger")]
    trigger: String,
}

impl Notification {
    #[must_use]
    pub fn new(kind: NotificationKind, before_start: Duration) -> Notification {
        Notification {
            kind,
            trigger: Duration {
                negative: true,
                ..before_start
            }
            .to_string(),
        }
    }

    #[must_use]
    pub fn with_trigger(kind: NotificationKind, trigger: Duration) -> Notification {
        Notification {
            kind,
            trigger: trigger.to_string(),
        }
    }

    pub fn from_alarm(
        alarm: &Alarm,
        start: &DateValue,
        zones: &Zones,
        floating: &TimeZone,
    ) -> Result<Notification> {
        let kind = match alarm.action() {
            AlarmAction::Email => NotificationKind::Email,
            _ => NotificationKind::Device,
        };
        let duration = match alarm.trigger()? {
            Trigger::Relative { duration, .. } => duration,
            Trigger::Absolute(at) => {
                let begin = start.timestamp(zones, floating)?.as_second();
                let fired = at.timestamp(zones, floating)?.as_second();
                Duration::from_seconds((fired - begin).div_euclid(60) * 60)
            }
        };
        Ok(Notification::with_trigger(
            kind,
            normalized(duration, start.is_date()),
        ))
    }

    #[must_use]
    pub fn kind(&self) -> NotificationKind {
        self.kind
    }

    #[must_use]
    pub fn trigger_text(&self) -> &str {
        &self.trigger
    }

    pub fn trigger(&self) -> Result<Duration> {
        self.trigger.parse()
    }

    pub fn to_alarm(&self) -> Result<Alarm> {
        let action = match self.kind {
            NotificationKind::Email => AlarmAction::Email,
            NotificationKind::Device => AlarmAction::Display,
        };
        Ok(Alarm::new(
            action,
            Trigger::Relative {
                duration: self.trigger()?,
                from_end: false,
            },
        ))
    }
}

fn normalized(duration: Duration, all_day: bool) -> Duration {
    let duration = Duration {
        seconds: 0,
        ..duration
    };
    if all_day {
        let midnight = duration.hours == 0 && duration.minutes == 0;
        let keep_weeks = if !duration.negative || midnight {
            duration.days == 0
        } else {
            duration.days == 6
        };
        if keep_weeks {
            return duration;
        }
        return Duration {
            weeks: 0,
            days: duration.days + 7 * duration.weeks,
            ..duration
        };
    }
    let parts = [
        duration.minutes,
        duration.hours,
        duration.weeks,
        duration.days,
    ]
    .iter()
    .filter(|part| **part > 0)
    .count();
    if parts <= 1 {
        return duration;
    }
    let minutes = duration.total_seconds().abs() / 60;
    let mut single = Duration {
        negative: duration.negative,
        ..Duration::default()
    };
    if minutes % 60 != 0 {
        single.minutes = minutes as u32;
    } else if (minutes / 60) % 24 != 0 {
        single.hours = (minutes / 60) as u32;
    } else if (minutes / 1440) % 7 != 0 {
        single.days = (minutes / 1440) as u32;
    } else {
        single.weeks = (minutes / 10_080) as u32;
    }
    single
}

pub fn alarm_times(
    alarm: &Alarm,
    instance: &Instance,
    zones: &Zones,
    floating: &TimeZone,
) -> Result<Vec<Timestamp>> {
    let first = match alarm.trigger()? {
        Trigger::Absolute(at) => at.timestamp(zones, floating)?,
        Trigger::Relative {
            duration,
            from_end: true,
        } => instance
            .end
            .checked_add(jiff::SignedDuration::from_secs(duration.total_seconds()))?,
        Trigger::Relative { duration, .. } => {
            let tzid = match &instance.local_start {
                DateValue::Zoned { tzid, .. } => Some(tzid.as_str()),
                DateValue::Utc(_) => Some("UTC"),
                _ => None,
            };
            let local = instance.local_start.local_seconds() + duration.nominal_days() * DAY;
            zones
                .local_to_timestamp(local, tzid, floating)?
                .checked_add(jiff::SignedDuration::from_secs(duration.exact_seconds()))?
        }
    };

    let mut times = vec![first];
    if let Some((count, every)) = alarm.repeat() {
        let step = every.total_seconds().abs();
        if step > 0 {
            for index in 1..=i64::from(count) {
                times.push(first.checked_add(jiff::SignedDuration::from_secs(step * index))?);
            }
        }
    }
    Ok(times)
}
