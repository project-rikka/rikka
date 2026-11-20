use std::rc::Rc;
use std::time::{Duration, SystemTime};

use rikka_app::prelude::*;
use rikka_battery::{Battery, DeviceId, Sample};
use rikka_power::Power;
use rikka_power_profiles::{PowerProfiles, Profile};
use rikka_settings::session::power::DOMAIN;

use crate::domain::{Domain, domain_page};
use crate::page::Page;
use crate::pending::Pending;
use crate::power::buttons;
use crate::session::Session;

const FOLLOWING_POWER: &str = "power-following-logind";
const FOLLOWING_BATTERY: &str = "power-following-battery";
const FOLLOWING_PROFILES: &str = "power-following-profiles";

pub(crate) const HISTORY_SPAN: Duration = Duration::from_secs(24 * 60 * 60);
const HISTORY_POINTS: u32 = 288;
const HISTORY_STEP: f32 = 0.01;

pub(crate) struct PowerPage {
    domain: Domain,
    daemons: Option<Daemons>,
    power: rikka_power::State,
    battery: rikka_battery::State,
    profiles: rikka_power_profiles::State,
    history: Rc<Vec<Sample>>,
    read_at: SystemTime,
    read_charge: Option<f32>,
    lid: bool,
}

struct Daemons {
    power: Power,
    battery: Battery,
    profiles: PowerProfiles,
}

impl Page for PowerPage {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        self.lid = buttons::has_lid();
        let daemons = self.daemons.get_or_insert_with(|| Daemons {
            power: Power::start(),
            battery: Battery::start(),
            profiles: PowerProfiles::start(),
        });
        daemons.power.reread();
        self.power = daemons.power.state();
        self.battery = daemons.battery.state();
        self.history = Rc::new(self.battery.history.clone());
        self.profiles = daemons.profiles.state();
        stream_keyed(
            FOLLOWING_POWER,
            daemons.power.changes(),
            |page: &mut PowerPage, state: rikka_power::State| page.power = state,
        );
        stream_keyed(
            FOLLOWING_BATTERY,
            daemons.battery.changes(),
            |page: &mut PowerPage, state: rikka_battery::State| page.follow_battery(state),
        );
        stream_keyed(
            FOLLOWING_PROFILES,
            daemons.profiles.changes(),
            |page: &mut PowerPage, state: rikka_power_profiles::State| page.profiles = state,
        );
        self.read_history();
    }

    fn closed(&mut self) {
        cancel(FOLLOWING_POWER);
        cancel(FOLLOWING_BATTERY);
        cancel(FOLLOWING_PROFILES);
    }
}

domain_page!(PowerPage);

impl PowerPage {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
            daemons: None,
            power: rikka_power::State::default(),
            battery: rikka_battery::State::default(),
            profiles: rikka_power_profiles::State::default(),
            history: Rc::new(Vec::new()),
            read_at: SystemTime::now(),
            read_charge: None,
            lid: false,
        }
    }

    pub(crate) fn power(&self) -> &rikka_power::State {
        &self.power
    }

    pub(crate) fn battery(&self) -> &rikka_battery::State {
        &self.battery
    }

    pub(crate) fn profiles(&self) -> &rikka_power_profiles::State {
        &self.profiles
    }

    pub(crate) fn history(&self) -> Rc<Vec<Sample>> {
        Rc::clone(&self.history)
    }

    pub(crate) fn read_at(&self) -> SystemTime {
        self.read_at
    }

    pub(crate) fn has_lid(&self) -> bool {
        self.lid
    }

    pub(crate) fn brighten(&mut self, percent: f32) {
        if let Some(daemons) = &self.daemons {
            daemons.power.brighten(percent / 100.0);
        }
    }

    pub(crate) fn choose_profile(&mut self, profile: Profile) {
        if let Some(daemons) = &self.daemons {
            daemons.profiles.set(profile);
        }
    }

    pub(crate) fn preserve(&mut self, pack: DeviceId, on: bool) {
        if let Some(daemons) = &self.daemons {
            daemons.battery.preserve(&pack, on);
        }
    }

    fn follow_battery(&mut self, state: rikka_battery::State) {
        if state.history != self.battery.history {
            self.history = Rc::new(state.history.clone());
            self.read_at = SystemTime::now();
        }
        let charge = state.main.map(|reading| reading.charge);
        self.battery = state;
        let moved = match (charge, self.read_charge) {
            (Some(now), Some(then)) => (now - then).abs() >= HISTORY_STEP,
            (now, then) => now.is_some() != then.is_some(),
        };
        if moved {
            self.read_history();
        }
    }

    fn read_history(&mut self) {
        self.read_charge = self.battery.main.map(|reading| reading.charge);
        if let (Some(daemons), Some(_)) = (&self.daemons, self.battery.main) {
            daemons.battery.read_history(HISTORY_SPAN, HISTORY_POINTS);
        }
    }
}

impl std::fmt::Debug for PowerPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PowerPage")
            .field("battery", &self.battery.main)
            .finish_non_exhaustive()
    }
}
