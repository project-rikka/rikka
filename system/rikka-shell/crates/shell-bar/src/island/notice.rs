use rikka_power_profiles::Profile;
use rikka_privacy::{Sensor, User};
use rikka_ui::{Label, MaterialSymbol, tr};
use shell_activity::ActivityEvent;
use shell_feeds::{percent, symbols};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    Good,
    Warning,
    Danger,
}

#[derive(Clone, Debug)]
pub(crate) struct Notice {
    pub(crate) symbol: MaterialSymbol,
    pub(crate) tone: Tone,
    pub(crate) title: Label,
    pub(crate) detail: Option<Label>,
    pub(crate) level: Option<f32>,
}

#[must_use]
pub(super) fn shows_notice(activity_event: &ActivityEvent) -> bool {
    match activity_event {
        ActivityEvent::Playback { .. } => false,
        ActivityEvent::Sensor { active, .. } => *active,
        _ => true,
    }
}

impl Notice {
    fn new(symbol: MaterialSymbol, title: Label) -> Notice {
        Notice {
            symbol,
            tone: Tone::Plain,
            title,
            detail: None,
            level: None,
        }
    }

    fn tone(mut self, tone: Tone) -> Notice {
        self.tone = tone;
        self
    }

    fn detail(mut self, detail: impl Into<Label>) -> Notice {
        self.detail = Some(detail.into());
        self
    }

    fn level(mut self, level: f32) -> Notice {
        self.level = Some(level.clamp(0.0, 1.0));
        self
    }

    #[must_use]
    pub(super) fn from_event(activity_event: &ActivityEvent) -> Option<Notice> {
        Some(match activity_event {
            ActivityEvent::Volume {
                output,
                level,
                muted,
            } => Notice::new(
                symbols::loudness(*level, *muted),
                Label::from(output.as_str()),
            )
            .level(if *muted { 0.0 } else { *level }),
            ActivityEvent::Brightness { level } => Notice::new(
                MaterialSymbol::BRIGHTNESS_MEDIUM,
                tr!("shell.island.brightness"),
            )
            .level(*level),
            ActivityEvent::Output { title, .. } => {
                Notice::new(MaterialSymbol::HEADPHONES, Label::from(title.as_str()))
                    .detail(tr!("shell.island.playing-through"))
            }
            ActivityEvent::Microphone { muted: true } => Notice::new(
                MaterialSymbol::MIC_OFF,
                tr!("shell.island.microphone-muted"),
            )
            .tone(Tone::Warning),
            ActivityEvent::Microphone { muted: false } => {
                Notice::new(MaterialSymbol::MIC, tr!("shell.island.microphone-live"))
            }
            ActivityEvent::Charger { plugged, charge } => {
                let notice = match plugged {
                    true => Notice::new(MaterialSymbol::BOLT, tr!("shell.island.plugged"))
                        .tone(Tone::Good),
                    false => Notice::new(MaterialSymbol::POWER_OFF, tr!("shell.island.unplugged")),
                };
                match charge {
                    Some(charge) => notice.detail(percent(*charge)),
                    None => notice,
                }
            }
            ActivityEvent::BatteryLow { charge, .. } => Notice::new(
                MaterialSymbol::BATTERY_ALERT,
                tr!("shell.island.battery-low"),
            )
            .tone(Tone::Danger)
            .detail(percent(*charge)),
            ActivityEvent::Charged => {
                Notice::new(MaterialSymbol::BATTERY_FULL, tr!("shell.island.charged"))
                    .tone(Tone::Good)
            }
            ActivityEvent::PeripheralLow { device } => {
                let notice = Notice::new(
                    MaterialSymbol::BATTERY_ALERT,
                    Label::from(device.name.as_str()),
                )
                .tone(Tone::Warning);
                match device.charge {
                    Some(charge) => notice.detail(tr!(
                        "shell.island.running-low",
                        charge = percent(charge).as_str()
                    )),
                    None => notice.detail(tr!("shell.island.battery-low")),
                }
            }
            ActivityEvent::Bluetooth { device, connected } => {
                let notice = Notice::new(
                    symbols::device(device.kind),
                    Label::from(device.title.as_str()),
                );
                let notice = match connected {
                    true => notice
                        .tone(Tone::Good)
                        .detail(tr!("shell.island.connected")),
                    false => notice.detail(tr!("shell.island.disconnected")),
                };
                match (connected, device.battery) {
                    (true, Some(battery)) => notice.level(f32::from(battery) / 100.0),
                    _ => notice,
                }
            }
            ActivityEvent::PowerProfile { profile } => {
                Notice::new(profile_symbol(*profile), tr!("shell.island.power-mode"))
                    .detail(tr!(&format!("shell.island.profile.{}", profile.name())))
            }
            ActivityEvent::Sensor { sensor, users, .. } => {
                Notice::new(sensor_symbol(*sensor), sensor_name(*sensor))
                    .tone(sensor_tone(*sensor))
                    .detail(who(users))
            }
            ActivityEvent::Online { connection } => {
                let notice =
                    Notice::new(MaterialSymbol::WIFI, tr!("shell.island.online")).tone(Tone::Good);
                match connection {
                    Some(connection) => notice.detail(Label::from(connection.as_str())),
                    None => notice,
                }
            }
            ActivityEvent::Offline => {
                Notice::new(MaterialSymbol::PUBLIC_OFF, tr!("shell.island.offline"))
                    .tone(Tone::Warning)
            }
            ActivityEvent::Vpn { name, connected } => match connected {
                true => Notice::new(MaterialSymbol::VPN_LOCK, tr!("shell.island.vpn-on"))
                    .tone(Tone::Good)
                    .detail(Label::from(name.as_str())),
                false => Notice::new(MaterialSymbol::VPN_KEY, tr!("shell.island.vpn-off"))
                    .detail(Label::from(name.as_str())),
            },
            ActivityEvent::Track { .. } | ActivityEvent::Playback { .. } => return None,
        })
    }
}

#[must_use]
pub(super) fn sensor_symbol(sensor: Sensor) -> MaterialSymbol {
    match sensor {
        Sensor::Microphone => MaterialSymbol::MIC,
        Sensor::Camera => MaterialSymbol::VIDEOCAM,
        Sensor::Screen => MaterialSymbol::SCREEN_SHARE,
    }
}

#[must_use]
pub(super) fn sensor_tone(sensor: Sensor) -> Tone {
    match sensor {
        Sensor::Microphone => Tone::Warning,
        Sensor::Camera => Tone::Good,
        Sensor::Screen => Tone::Danger,
    }
}

#[must_use]
pub(super) fn sensor_name(sensor: Sensor) -> Label {
    match sensor {
        Sensor::Microphone => tr!("shell.island.microphone"),
        Sensor::Camera => tr!("shell.island.camera"),
        Sensor::Screen => tr!("shell.island.screen"),
    }
}

#[must_use]
pub(crate) fn who(users: &[User]) -> Label {
    let mut names: Vec<&str> = users.iter().map(|user| user.name.as_str()).collect();
    names.dedup();
    match names.is_empty() {
        true => tr!("shell.island.in-use"),
        false => Label::from(names.join(", ")),
    }
}

fn profile_symbol(profile: Profile) -> MaterialSymbol {
    match profile {
        Profile::PowerSaver => MaterialSymbol::ENERGY_SAVINGS_LEAF,
        Profile::Balanced => MaterialSymbol::BALANCE,
        Profile::Performance => MaterialSymbol::SPEED,
    }
}
