use rikka_battery::Warning;
use rikka_mpris::{PlayerId, Track};
use rikka_power_profiles::Profile;
use rikka_privacy::{Sensor, User};

#[derive(Clone, Debug, PartialEq)]
pub enum ActivityEvent {
    Volume {
        output: String,
        level: f32,
        muted: bool,
    },
    Output {
        name: String,
        title: String,
        icon: Option<String>,
    },
    Microphone {
        muted: bool,
    },
    Brightness {
        level: f32,
    },
    Charger {
        plugged: bool,
        charge: Option<f32>,
    },
    BatteryLow {
        charge: f32,
        warning: Warning,
    },
    Charged,
    PeripheralLow {
        device: rikka_battery::Device,
    },
    Bluetooth {
        device: rikka_bluetooth::Device,
        connected: bool,
    },
    Track {
        player: PlayerId,
        track: Track,
    },
    Playback {
        player: PlayerId,
        playing: bool,
    },
    PowerProfile {
        profile: Profile,
    },
    Sensor {
        sensor: Sensor,
        users: Vec<User>,
        active: bool,
    },
    Online {
        connection: Option<String>,
    },
    Offline,
    Vpn {
        name: String,
        connected: bool,
    },
}
