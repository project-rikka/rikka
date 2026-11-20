use std::collections::BTreeSet;

use rikka_battery::{Charge, Level, Warning};
use rikka_network::{ActiveState, Connectivity};
use rikka_privacy::Sensor;

use crate::activity::{self, Activity};
use crate::activity_event::ActivityEvent;

const NUDGE: f32 = 0.001;
const PERIPHERAL_LOW: f32 = 0.1;

#[derive(Clone, Debug, PartialEq)]
struct Output {
    name: String,
    title: String,
    icon: Option<String>,
    level: f32,
    muted: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct SoundState {
    sink: Option<Output>,
    microphone_muted: Option<bool>,
}

impl SoundState {
    fn from_snapshot(graph: &rikka_sound::Snapshot) -> SoundState {
        SoundState {
            sink: graph.sink().map(|sink| Output {
                name: sink.name.clone(),
                title: sink.title.clone(),
                icon: sink.icon.clone(),
                level: sink.volume.level(),
                muted: sink.volume.muted(),
            }),
            microphone_muted: graph.source().map(|source| source.volume.muted()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reach {
    connectivity: Connectivity,
    primary: Option<String>,
    tunnels: BTreeSet<String>,
}

impl Reach {
    fn from_state(state: &rikka_network::State) -> Reach {
        let up = |active: &&rikka_network::Active| active.state == ActiveState::Activated;
        let active = &state.snapshot.active;
        Reach {
            connectivity: state.snapshot.connectivity,
            primary: active
                .iter()
                .filter(up)
                .find(|active| active.default4 || active.default6)
                .map(|active| active.title.clone()),
            tunnels: active
                .iter()
                .filter(up)
                .filter(|active| active.vpn)
                .map(|active| active.title.clone())
                .collect(),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ActivityTracker {
    sound: Option<SoundState>,
    battery: Option<rikka_battery::State>,
    bluetooth: Option<Vec<rikka_bluetooth::Device>>,
    brightness: Option<Option<f32>>,
    profile: Option<Option<rikka_power_profiles::Profile>>,
    media: Option<rikka_mpris::State>,
    privacy: Option<rikka_privacy::State>,
    reach: Option<Reach>,
}

impl ActivityTracker {
    pub(super) fn sound(&mut self, graph: &rikka_sound::Snapshot) -> Vec<ActivityEvent> {
        let now = SoundState::from_snapshot(graph);
        let Some(previous) = self.sound.replace(now.clone()) else {
            return Vec::new();
        };
        let mut activity_events = Vec::new();
        if let (Some(was), Some(is)) = (&previous.sink, &now.sink) {
            if was.name != is.name {
                activity_events.push(ActivityEvent::Output {
                    name: is.name.clone(),
                    title: is.title.clone(),
                    icon: is.icon.clone(),
                });
            } else if (was.level - is.level).abs() > NUDGE || was.muted != is.muted {
                activity_events.push(ActivityEvent::Volume {
                    output: is.title.clone(),
                    level: is.level,
                    muted: is.muted,
                });
            }
        }
        if let (Some(was), Some(is)) = (previous.microphone_muted, now.microphone_muted)
            && was != is
        {
            activity_events.push(ActivityEvent::Microphone { muted: is });
        }
        activity_events
    }

    pub(super) fn battery(&mut self, now: rikka_battery::State) -> Vec<ActivityEvent> {
        let previous = self.battery.replace(now);
        match (previous, &self.battery) {
            (Some(previous), Some(now)) => battery_events(&previous, now),
            _ => Vec::new(),
        }
    }

    pub(super) fn bluetooth(&mut self, now: &rikka_bluetooth::State) -> Vec<ActivityEvent> {
        let devices = now.snapshot.devices.clone();
        let Some(previous) = self.bluetooth.replace(devices.clone()) else {
            return Vec::new();
        };
        devices
            .into_iter()
            .filter(|device| {
                previous
                    .iter()
                    .find(|was| was.id == device.id)
                    .is_some_and(|was| was.connected != device.connected)
            })
            .map(|device| ActivityEvent::Bluetooth {
                connected: device.connected,
                device,
            })
            .collect()
    }

    pub(super) fn power(&mut self, now: &rikka_power::State) -> Vec<ActivityEvent> {
        let Some(previous) = self.brightness.replace(now.brightness) else {
            return Vec::new();
        };
        match (previous, now.brightness) {
            (Some(was), Some(is)) if (was - is).abs() > NUDGE => {
                vec![ActivityEvent::Brightness { level: is }]
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn profiles(&mut self, now: &rikka_power_profiles::State) -> Vec<ActivityEvent> {
        let Some(previous) = self.profile.replace(now.active) else {
            return Vec::new();
        };
        match (previous, now.active) {
            (Some(was), Some(is)) if was != is => vec![ActivityEvent::PowerProfile { profile: is }],
            _ => Vec::new(),
        }
    }

    pub(super) fn media(&mut self, now: rikka_mpris::State) -> Vec<ActivityEvent> {
        let previous = self.media.replace(now);
        match (previous, &self.media) {
            (Some(previous), Some(now)) => media_events(&previous, now),
            _ => Vec::new(),
        }
    }

    pub(super) fn privacy(&mut self, now: rikka_privacy::State) -> Vec<ActivityEvent> {
        let previous = self.privacy.replace(now);
        match (previous, &self.privacy) {
            (Some(previous), Some(now)) => privacy_events(&previous, now),
            _ => Vec::new(),
        }
    }

    pub(super) fn activities(&self) -> Vec<Activity> {
        activity::current(
            self.privacy
                .as_ref()
                .unwrap_or(&rikka_privacy::State::default()),
            self.media
                .as_ref()
                .unwrap_or(&rikka_mpris::State::default()),
            self.battery
                .as_ref()
                .unwrap_or(&rikka_battery::State::default()),
        )
    }

    pub(super) fn network(&mut self, now: &rikka_network::State) -> Vec<ActivityEvent> {
        let reach = Reach::from_state(now);
        let Some(previous) = self.reach.replace(reach.clone()) else {
            return Vec::new();
        };
        let mut activity_events = Vec::new();
        let reached = |connectivity| connectivity == Connectivity::Full;
        let cut = |connectivity| connectivity == Connectivity::None;
        if reached(reach.connectivity) && !reached(previous.connectivity) {
            activity_events.push(ActivityEvent::Online {
                connection: reach.primary.clone(),
            });
        }
        if cut(reach.connectivity)
            && !cut(previous.connectivity)
            && previous.connectivity != Connectivity::Unknown
        {
            activity_events.push(ActivityEvent::Offline);
        }
        for name in reach.tunnels.difference(&previous.tunnels) {
            activity_events.push(ActivityEvent::Vpn {
                name: name.clone(),
                connected: true,
            });
        }
        for name in previous.tunnels.difference(&reach.tunnels) {
            activity_events.push(ActivityEvent::Vpn {
                name: name.clone(),
                connected: false,
            });
        }
        activity_events
    }
}

fn battery_events(
    previous: &rikka_battery::State,
    now: &rikka_battery::State,
) -> Vec<ActivityEvent> {
    let mut activity_events = Vec::new();
    if previous.on_battery != now.on_battery && now.main.is_some() {
        activity_events.push(ActivityEvent::Charger {
            plugged: !now.on_battery,
            charge: now.main.map(|main| main.charge),
        });
    }
    if let (Some(was), Some(is)) = (previous.main, now.main) {
        if is.warning > was.warning && is.warning >= Warning::Low {
            activity_events.push(ActivityEvent::BatteryLow {
                charge: is.charge,
                warning: is.warning,
            });
        }
        if is.state == Charge::Full && was.state == Charge::Charging {
            activity_events.push(ActivityEvent::Charged);
        }
    }
    for device in &now.devices {
        let Some(was) = previous.devices.iter().find(|was| was.id == device.id) else {
            continue;
        };
        let warned = device.warning > was.warning && device.warning >= Warning::Low;
        let sank = device.level < was.level
            && device.level != Level::Unknown
            && device.level <= Level::Low;
        let drained = matches!(
            (was.charge, device.charge),
            (Some(was), Some(is)) if was > PERIPHERAL_LOW && is <= PERIPHERAL_LOW
        );
        if warned || sank || drained {
            activity_events.push(ActivityEvent::PeripheralLow {
                device: device.clone(),
            });
        }
    }
    activity_events
}

fn media_events(previous: &rikka_mpris::State, now: &rikka_mpris::State) -> Vec<ActivityEvent> {
    let Some(is) = now.active() else {
        return Vec::new();
    };
    let track_id =
        |player: &rikka_mpris::Player| player.track.as_ref().map(|track| track.id.clone());
    let mut activity_events = Vec::new();
    match previous.player(&is.id) {
        Some(was) => {
            if was.status != is.status && (was.is_playing() || is.is_playing()) {
                activity_events.push(ActivityEvent::Playback {
                    player: is.id.clone(),
                    playing: is.is_playing(),
                });
            }
            if is.is_playing()
                && track_id(was) != track_id(is)
                && let Some(track) = &is.track
            {
                activity_events.push(ActivityEvent::Track {
                    player: is.id.clone(),
                    track: track.clone(),
                });
            }
        }
        None => {
            if is.is_playing()
                && let Some(track) = &is.track
            {
                activity_events.push(ActivityEvent::Track {
                    player: is.id.clone(),
                    track: track.clone(),
                });
            }
        }
    }
    activity_events
}

fn privacy_events(
    previous: &rikka_privacy::State,
    now: &rikka_privacy::State,
) -> Vec<ActivityEvent> {
    let mut activity_events = Vec::new();
    for sensor in [Sensor::Screen, Sensor::Camera, Sensor::Microphone] {
        let (was, is) = (previous.users(sensor), now.users(sensor));
        if is.is_empty() && !was.is_empty() {
            activity_events.push(ActivityEvent::Sensor {
                sensor,
                users: was.to_vec(),
                active: false,
            });
            continue;
        }
        let joined: Vec<_> = is
            .iter()
            .filter(|user| !was.contains(user))
            .cloned()
            .collect();
        if !joined.is_empty() {
            activity_events.push(ActivityEvent::Sensor {
                sensor,
                users: joined,
                active: true,
            });
        }
    }
    activity_events
}
