use std::sync::Arc;

use rikka_battery::Battery;
use rikka_bluetooth::Bluetooth;
use rikka_network::Network;
use rikka_power::Power;
use rikka_sound::Sound;

#[derive(Clone, Debug)]
pub struct Daemons(Arc<Parts>);

impl Daemons {
    #[must_use]
    pub fn start() -> Daemons {
        Daemons(Arc::new(Parts {
            sound: Sound::start(),
            network: Network::start(),
            bluetooth: Bluetooth::start(),
            power: Power::start(),
            battery: Battery::start(),
        }))
    }

    #[must_use]
    pub fn sound(&self) -> &Sound {
        &self.0.sound
    }

    #[must_use]
    pub fn network(&self) -> &Network {
        &self.0.network
    }

    #[must_use]
    pub fn bluetooth(&self) -> &Bluetooth {
        &self.0.bluetooth
    }

    #[must_use]
    pub fn power(&self) -> &Power {
        &self.0.power
    }

    #[must_use]
    pub fn battery(&self) -> &Battery {
        &self.0.battery
    }
}

#[derive(Debug)]
struct Parts {
    sound: Sound,
    network: Network,
    bluetooth: Bluetooth,
    power: Power,
    battery: Battery,
}
