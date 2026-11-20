use rikka_battery::{Charge, Reading};
use rikka_bluetooth::DeviceKind;
use rikka_network::{AccessPoint, Bars};
use rikka_sound::Volume;
use rikka_ui::MaterialSymbol;

pub fn sound(volume: Option<&Volume>) -> MaterialSymbol {
    match volume {
        Some(volume) => loudness(volume.level(), volume.muted()),
        None => MaterialSymbol::VOLUME_OFF,
    }
}

pub fn loudness(level: f32, muted: bool) -> MaterialSymbol {
    match level {
        _ if muted => MaterialSymbol::VOLUME_OFF,
        level if level <= 0.0 => MaterialSymbol::VOLUME_MUTE,
        level if level < 0.5 => MaterialSymbol::VOLUME_DOWN,
        _ => MaterialSymbol::VOLUME_UP,
    }
}

pub fn microphone(volume: Option<&Volume>) -> MaterialSymbol {
    match volume.is_some_and(|volume| !volume.muted()) {
        true => MaterialSymbol::MIC,
        false => MaterialSymbol::MIC_OFF,
    }
}

pub fn wireless(on: bool, joined: Option<&AccessPoint>) -> MaterialSymbol {
    if !on {
        return MaterialSymbol::WIFI_OFF;
    }
    match joined {
        None => MaterialSymbol::SIGNAL_WIFI_0_BAR,
        Some(point) => bars(point.bars()),
    }
}

pub fn bars(bars: Bars) -> MaterialSymbol {
    match bars {
        Bars::Four => MaterialSymbol::WIFI,
        Bars::Three => MaterialSymbol::NETWORK_WIFI_3_BAR,
        Bars::Two => MaterialSymbol::WIFI_2_BAR,
        Bars::One => MaterialSymbol::WIFI_1_BAR,
    }
}

pub fn bluetooth<T>(on: bool, connected: Option<T>) -> MaterialSymbol {
    match (on, connected.is_some()) {
        (false, _) => MaterialSymbol::BLUETOOTH_DISABLED,
        (true, false) => MaterialSymbol::BLUETOOTH,
        (true, true) => MaterialSymbol::BLUETOOTH_CONNECTED,
    }
}

pub fn battery(battery: &Reading) -> MaterialSymbol {
    match battery.state {
        Charge::Charging => MaterialSymbol::BATTERY_CHARGING_FULL,
        Charge::Full => MaterialSymbol::BATTERY_FULL,
        _ if battery.charge <= 0.1 => MaterialSymbol::BATTERY_ALERT,
        _ => bars_of(battery.charge),
    }
}

fn bars_of(charge: f32) -> MaterialSymbol {
    match (charge * 6.0).round() as u8 {
        0 => MaterialSymbol::BATTERY_0_BAR,
        1 => MaterialSymbol::BATTERY_1_BAR,
        2 => MaterialSymbol::BATTERY_2_BAR,
        3 => MaterialSymbol::BATTERY_3_BAR,
        4 => MaterialSymbol::BATTERY_4_BAR,
        5 => MaterialSymbol::BATTERY_5_BAR,
        _ => MaterialSymbol::BATTERY_6_BAR,
    }
}

pub fn device(kind: DeviceKind) -> MaterialSymbol {
    match kind {
        DeviceKind::Computer => MaterialSymbol::COMPUTER,
        DeviceKind::Phone => MaterialSymbol::SMARTPHONE,
        DeviceKind::Headset => MaterialSymbol::HEADSET,
        DeviceKind::Headphones => MaterialSymbol::HEADPHONES,
        DeviceKind::Speaker => MaterialSymbol::SPEAKER,
        DeviceKind::Keyboard => MaterialSymbol::KEYBOARD,
        DeviceKind::Mouse => MaterialSymbol::MOUSE,
        DeviceKind::Tablet => MaterialSymbol::TABLET_MAC,
        DeviceKind::Gamepad => MaterialSymbol::SPORTS_ESPORTS,
        DeviceKind::Watch => MaterialSymbol::WATCH,
        DeviceKind::Printer => MaterialSymbol::PRINT,
        DeviceKind::Camera => MaterialSymbol::PHOTO_CAMERA,
        DeviceKind::Display => MaterialSymbol::TV,
        DeviceKind::Network => MaterialSymbol::LAN,
        DeviceKind::Health => MaterialSymbol::MONITOR_HEART,
        DeviceKind::Unknown => MaterialSymbol::DEVICES_OTHER,
    }
}
