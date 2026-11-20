use serde::{Deserialize, Serialize};

use crate::widget::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "u32", into = "u32")]
pub enum PhotoInterval {
    HalfMinute,
    Minute,
    FiveMinutes,
    HalfHour,
    Hour,
}

impl PhotoInterval {
    pub const ALL: [PhotoInterval; 5] = [
        PhotoInterval::HalfMinute,
        PhotoInterval::Minute,
        PhotoInterval::FiveMinutes,
        PhotoInterval::HalfHour,
        PhotoInterval::Hour,
    ];

    #[must_use]
    pub const fn secs(self) -> u32 {
        match self {
            PhotoInterval::HalfMinute => 30,
            PhotoInterval::Minute => 60,
            PhotoInterval::FiveMinutes => 300,
            PhotoInterval::HalfHour => 1800,
            PhotoInterval::Hour => 3600,
        }
    }

    #[must_use]
    pub fn nearest(secs: u32) -> PhotoInterval {
        let mut nearest = PhotoInterval::HalfMinute;
        for interval in PhotoInterval::ALL {
            if interval.secs().abs_diff(secs) < nearest.secs().abs_diff(secs) {
                nearest = interval;
            }
        }
        nearest
    }
}

impl From<u32> for PhotoInterval {
    fn from(secs: u32) -> PhotoInterval {
        PhotoInterval::nearest(secs)
    }
}

impl From<PhotoInterval> for u32 {
    fn from(interval: PhotoInterval) -> u32 {
        interval.secs()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Content {
    Clock(Clock),
    Calendar(Calendar),
    Weather(Weather),
    Media(Media),
    Monitor(Monitor),
    Power(Power),
    Note(Note),
    Photos(Photos),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Clock {
    pub face: Face,
    pub seconds: bool,
    pub zone: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Face {
    #[default]
    Digital,
    Analog,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Calendar {
    pub first_day: FirstDay,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FirstDay {
    #[default]
    Locale,
    Saturday,
    Sunday,
    Monday,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Weather {
    pub air: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Media {
    pub waveform: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Monitor {
    pub meters: Vec<Meter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Meter {
    Cpu,
    Memory,
    Gpu,
    Network,
    Disk,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Power {
    pub devices: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Note {
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Photos {
    pub folder: Option<String>,
    pub interval: PhotoInterval,
    pub shuffle: bool,
    pub fill: bool,
}

impl Content {
    #[must_use]
    pub fn new(kind: Kind) -> Content {
        match kind {
            Kind::Clock => Content::Clock(Clock::default()),
            Kind::Calendar => Content::Calendar(Calendar::default()),
            Kind::Weather => Content::Weather(Weather::default()),
            Kind::Media => Content::Media(Media::default()),
            Kind::Monitor => Content::Monitor(Monitor::default()),
            Kind::Power => Content::Power(Power::default()),
            Kind::Note => Content::Note(Note::default()),
            Kind::Photos => Content::Photos(Photos::default()),
        }
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Content::Clock(_) => Kind::Clock,
            Content::Calendar(_) => Kind::Calendar,
            Content::Weather(_) => Kind::Weather,
            Content::Media(_) => Kind::Media,
            Content::Monitor(_) => Kind::Monitor,
            Content::Power(_) => Kind::Power,
            Content::Note(_) => Kind::Note,
            Content::Photos(_) => Kind::Photos,
        }
    }

    pub(super) fn sanitized(self) -> Content {
        match self {
            Content::Clock(clock) => Content::Clock(Clock {
                zone: clock
                    .zone
                    .map(|zone| zone.trim().to_owned())
                    .filter(|zone| !zone.is_empty()),
                ..clock
            }),
            Content::Monitor(monitor) => Content::Monitor(monitor.sanitized()),
            other => other,
        }
    }
}

impl Default for Weather {
    fn default() -> Weather {
        Weather { air: true }
    }
}

impl Default for Media {
    fn default() -> Media {
        Media { waveform: true }
    }
}

impl Default for Monitor {
    fn default() -> Monitor {
        Monitor {
            meters: vec![Meter::Cpu, Meter::Memory, Meter::Gpu, Meter::Network],
        }
    }
}

impl Monitor {
    fn sanitized(self) -> Monitor {
        let mut meters: Vec<Meter> = Vec::with_capacity(self.meters.len());
        for meter in self.meters {
            if !meters.contains(&meter) {
                meters.push(meter);
            }
        }
        match meters.is_empty() {
            true => Monitor::default(),
            false => Monitor { meters },
        }
    }
}

impl Default for Power {
    fn default() -> Power {
        Power { devices: true }
    }
}

impl Default for Photos {
    fn default() -> Photos {
        Photos {
            folder: None,
            interval: PhotoInterval::FiveMinutes,
            shuffle: true,
            fill: true,
        }
    }
}
