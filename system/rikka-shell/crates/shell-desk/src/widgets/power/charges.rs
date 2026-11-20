use std::cell::RefCell;

use rikka_battery::{Charge, DeviceKind, Level, Reading};
use rikka_ui::MaterialSymbol;
use shell_feeds::symbols;
use shell_kit::Feed;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Charges {
    pub(crate) main: Option<Reading>,
    pub(crate) on_battery: bool,
    pub(super) gadgets: Vec<PowerDevice>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PowerDevice {
    pub(crate) name: String,
    pub(crate) symbol: MaterialSymbol,
    pub(crate) charge: f32,
    pub(crate) charging: bool,
}

#[derive(Default)]
pub(crate) struct Charging {
    battery: RefCell<rikka_battery::State>,
    paired: RefCell<Vec<PowerDevice>>,
    charges: Feed<Charges>,
}

impl Charging {
    #[must_use]
    pub(super) fn charges(&self) -> Charges {
        self.charges.get()
    }

    pub(crate) fn on_battery(&self, state: rikka_battery::State) {
        *self.battery.borrow_mut() = state;
        self.settle();
    }

    pub(crate) fn on_bluetooth(&self, state: &rikka_bluetooth::State) {
        *self.paired.borrow_mut() = state
            .snapshot
            .devices
            .iter()
            .filter(|device| device.connected)
            .filter_map(|device| {
                Some(PowerDevice {
                    name: device.title.clone(),
                    symbol: symbols::device(device.kind),
                    charge: f32::from(device.battery?) / 100.0,
                    charging: false,
                })
            })
            .collect();
        self.settle();
    }

    fn settle(&self) {
        let battery = self.battery.borrow();
        let mut gadgets: Vec<PowerDevice> = battery
            .devices
            .iter()
            .filter(|device| !matches!(device.kind, DeviceKind::Computer | DeviceKind::Ups))
            .filter_map(|device| {
                Some(PowerDevice {
                    name: device.name.clone(),
                    symbol: symbol(device.kind),
                    charge: device.charge.or_else(|| roughly(device.level))?,
                    charging: device.state == Charge::Charging,
                })
            })
            .collect();
        for paired in self.paired.borrow().iter() {
            let known = gadgets
                .iter()
                .any(|gadget| gadget.name.eq_ignore_ascii_case(&paired.name));
            if !known {
                gadgets.push(paired.clone());
            }
        }
        self.charges.set(Charges {
            main: battery.main,
            on_battery: battery.on_battery,
            gadgets,
        });
    }
}

fn roughly(level: Level) -> Option<f32> {
    match level {
        Level::Unknown => None,
        Level::Critical => Some(0.05),
        Level::Low => Some(0.2),
        Level::Normal => Some(0.55),
        Level::High => Some(0.8),
        Level::Full => Some(1.0),
    }
}

#[must_use]
pub(crate) fn symbol(kind: DeviceKind) -> MaterialSymbol {
    match kind {
        DeviceKind::Mouse => MaterialSymbol::MOUSE,
        DeviceKind::Keyboard => MaterialSymbol::KEYBOARD,
        DeviceKind::Touchpad => MaterialSymbol::TOUCH_APP,
        DeviceKind::Pen => MaterialSymbol::STYLUS,
        DeviceKind::Tablet => MaterialSymbol::TABLET_MAC,
        DeviceKind::Phone => MaterialSymbol::SMARTPHONE,
        DeviceKind::Headset => MaterialSymbol::HEADSET,
        DeviceKind::Headphones => MaterialSymbol::HEADPHONES,
        DeviceKind::Speakers => MaterialSymbol::SPEAKER,
        DeviceKind::GamingInput => MaterialSymbol::SPORTS_ESPORTS,
        DeviceKind::Wearable => MaterialSymbol::WATCH,
        DeviceKind::RemoteControl => MaterialSymbol::SETTINGS_REMOTE,
        DeviceKind::MediaPlayer => MaterialSymbol::MUSIC_NOTE,
        DeviceKind::Computer => MaterialSymbol::COMPUTER,
        DeviceKind::Ups => MaterialSymbol::POWER,
        DeviceKind::Other => MaterialSymbol::DEVICES_OTHER,
    }
}
