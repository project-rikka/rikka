use rikka_app::prelude::*;

use rikka_bluetooth::DeviceKind;

pub(crate) fn symbol(kind: DeviceKind) -> MaterialSymbol {
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

pub(crate) fn label(kind: DeviceKind) -> Label {
    match kind {
        DeviceKind::Computer => tr!("settings.bluetooth.kind.computer"),
        DeviceKind::Phone => tr!("settings.bluetooth.kind.phone"),
        DeviceKind::Headset => tr!("settings.bluetooth.kind.headset"),
        DeviceKind::Headphones => tr!("settings.bluetooth.kind.headphones"),
        DeviceKind::Speaker => tr!("settings.bluetooth.kind.speaker"),
        DeviceKind::Keyboard => tr!("settings.bluetooth.kind.keyboard"),
        DeviceKind::Mouse => tr!("settings.bluetooth.kind.mouse"),
        DeviceKind::Tablet => tr!("settings.bluetooth.kind.tablet"),
        DeviceKind::Gamepad => tr!("settings.bluetooth.kind.gamepad"),
        DeviceKind::Watch => tr!("settings.bluetooth.kind.watch"),
        DeviceKind::Printer => tr!("settings.bluetooth.kind.printer"),
        DeviceKind::Camera => tr!("settings.bluetooth.kind.camera"),
        DeviceKind::Display => tr!("settings.bluetooth.kind.display"),
        DeviceKind::Network => tr!("settings.bluetooth.kind.network"),
        DeviceKind::Health => tr!("settings.bluetooth.kind.health"),
        DeviceKind::Unknown => tr!("settings.bluetooth.kind.unknown"),
    }
}

pub(crate) fn battery(percentage: u8) -> MaterialSymbol {
    match percentage {
        90..=100 => MaterialSymbol::BATTERY_FULL,
        70..=89 => MaterialSymbol::BATTERY_5_BAR,
        50..=69 => MaterialSymbol::BATTERY_4_BAR,
        30..=49 => MaterialSymbol::BATTERY_3_BAR,
        15..=29 => MaterialSymbol::BATTERY_2_BAR,
        _ => MaterialSymbol::BATTERY_ALERT,
    }
}
