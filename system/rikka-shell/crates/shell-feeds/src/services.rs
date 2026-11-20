use std::sync::{Arc, OnceLock};

use futures_util::{Stream, StreamExt};
use rikka_battery::Battery;
use rikka_bluetooth::Bluetooth;
use rikka_mpris::Players;
use rikka_network::{Connectivity, Network};
use rikka_power_profiles::PowerProfiles;
use rikka_privacy::Privacy;
use rikka_sysmon::Machine;
use rikka_weather::{Spot, Weather};
use shell_activity::{Activities, ActivityEvent, ActivityMonitor, Sources};

use crate::daemons::Daemons;

#[derive(Clone)]
pub struct Services(Arc<Parts>);

struct Parts {
    app: String,
    spot: Spot,
    daemons: Daemons,
    media: OnceLock<Players>,
    machine: OnceLock<Machine>,
    weather: OnceLock<Weather>,
    profiles: OnceLock<PowerProfiles>,
    activity_monitor: OnceLock<ActivityMonitor>,
}

impl Services {
    pub(crate) fn start(app: impl Into<String>, daemons: Daemons, spot: Spot) -> Services {
        Services(Arc::new(Parts {
            app: app.into(),
            spot,
            daemons,
            media: OnceLock::new(),
            machine: OnceLock::new(),
            weather: OnceLock::new(),
            profiles: OnceLock::new(),
            activity_monitor: OnceLock::new(),
        }))
    }

    pub fn battery(&self) -> &Battery {
        self.0.daemons.battery()
    }

    pub fn bluetooth(&self) -> &Bluetooth {
        self.0.daemons.bluetooth()
    }

    pub fn media(&self) -> &Players {
        self.0.media.get_or_init(Players::start)
    }

    pub fn profiles(&self) -> &PowerProfiles {
        self.0.profiles.get_or_init(PowerProfiles::start)
    }

    pub fn machine(&self) -> &Machine {
        self.0.machine.get_or_init(Machine::start)
    }

    pub fn weather(&self) -> &Weather {
        self.0.weather.get_or_init(|| {
            Weather::builder(self.0.app.clone())
                .spot(self.0.spot.clone())
                .online(online(self.0.daemons.network()))
                .start()
        })
    }

    pub fn activities(&self) -> Activities {
        self.activity_monitor().activities()
    }

    pub fn activity_events(&self) -> impl Stream<Item = ActivityEvent> + Send + 'static {
        self.activity_monitor().activity_events()
    }

    fn activity_monitor(&self) -> &ActivityMonitor {
        self.0.activity_monitor.get_or_init(|| {
            let daemons = &self.0.daemons;
            ActivityMonitor::start(Sources {
                sound: daemons.sound().clone(),
                battery: daemons.battery().clone(),
                bluetooth: daemons.bluetooth().clone(),
                power: daemons.power().clone(),
                network: daemons.network().clone(),
                profiles: self.profiles().clone(),
                media: self.media().clone(),
                privacy: Privacy::start(),
            })
        })
    }
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services")
            .field("app", &self.0.app)
            .field("media", &self.0.media.get().is_some())
            .field("machine", &self.0.machine.get().is_some())
            .field("weather", &self.0.weather.get().is_some())
            .field("following", &self.0.activity_monitor.get().is_some())
            .finish_non_exhaustive()
    }
}

fn online(network: &Network) -> impl Stream<Item = bool> + Send + 'static {
    let reachable = |state: rikka_network::State| match state.snapshot.connectivity {
        Connectivity::Unknown => None,
        Connectivity::Full => Some(true),
        Connectivity::None | Connectivity::Portal | Connectivity::Limited => Some(false),
    };
    futures_util::stream::once(std::future::ready(network.state()))
        .chain(network.changes())
        .filter_map(move |state| std::future::ready(reachable(state)))
}
