use std::time::{Duration, Instant};

use rikka_mpris::Player;
use rikka_privacy::{Sensor, User};
use shell_activity::Activity;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Live {
    Media(Box<Player>),
    Recording(Instant),
    Sensor { sensor: Sensor, users: Vec<User> },
    LowBattery { charge: f32 },
    Charging { charge: f32, left: Option<Duration> },
}

impl Live {
    fn rank(&self) -> u8 {
        match self {
            Live::Media(_) => 0,
            Live::Recording(_) => 1,
            Live::Sensor {
                sensor: Sensor::Screen,
                ..
            } => 2,
            Live::Sensor {
                sensor: Sensor::Camera,
                ..
            } => 3,
            Live::Sensor {
                sensor: Sensor::Microphone,
                ..
            } => 4,
            Live::LowBattery { .. } => 5,
            Live::Charging { .. } => 6,
        }
    }
}

#[must_use]
pub(crate) fn ranked(activities: &[Activity], recording: Option<Instant>) -> Vec<Live> {
    let mut live_activities: Vec<Live> = activities
        .iter()
        .filter_map(|activity| match activity {
            Activity::Media { player } if player.is_playing() => Some(Live::Media(player.clone())),
            Activity::Media { .. } => None,
            Activity::Sensor { sensor, users } => Some(Live::Sensor {
                sensor: *sensor,
                users: users.clone(),
            }),
            Activity::LowBattery { charge, .. } => Some(Live::LowBattery { charge: *charge }),
            Activity::Charging { charge, left } => Some(Live::Charging {
                charge: *charge,
                left: *left,
            }),
        })
        .chain(recording.map(Live::Recording))
        .collect();
    live_activities.sort_by_key(Live::rank);
    live_activities
}

#[must_use]
pub(crate) fn title(player: &Player) -> String {
    player
        .track
        .as_ref()
        .map(|track| track.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| player.identity.clone())
}

#[must_use]
pub(crate) fn player(activities: &[Activity]) -> Option<&Player> {
    activities.iter().find_map(|activity| match activity {
        Activity::Media { player } => Some(player.as_ref()),
        _ => None,
    })
}
