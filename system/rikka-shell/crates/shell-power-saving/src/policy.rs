use std::time::Duration;

use rikka_battery::{Battery, Warning};
use rikka_power::{Action, Power};
use rikka_power_profiles::{PowerProfiles, Profile};
use rikka_session::display_power;
use rikka_settings::session::power::{
    DIM_DEFAULT, SAVER_ON_LOW_BATTERY_DEFAULT, SCREEN_OFF_BATTERY_DEFAULT,
    SCREEN_OFF_PLUGGED_DEFAULT, SUSPEND_BATTERY_DEFAULT, SUSPEND_PLUGGED_DEFAULT,
};
use shell_kit::{Idle, Slot, idle_minutes};

const DIM_LEAD: Duration = Duration::from_secs(30);
const DIM_SHORTEST: Duration = Duration::from_secs(15);
const DIMMED: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Minutes {
    pub(super) plugged: i64,
    pub(super) battery: i64,
}

#[derive(Debug)]
struct Chosen {
    screen_off: Minutes,
    suspend: Minutes,
    dim: bool,
    saver_on_low_battery: bool,
}

#[derive(Debug)]
struct Timers {
    dim: Idle,
    screen_off: Idle,
    suspend: Idle,
}

#[derive(Debug)]
pub(super) struct Policy {
    chosen: Chosen,
    on_battery: bool,
    undimmed: Option<f32>,
    saver_replaced: Option<Profile>,
    timers: Timers,
    power: Power,
    battery: Battery,
    profiles: PowerProfiles,
}

impl Policy {
    pub(super) fn new(
        idles: [Idle; 3],
        power: Power,
        battery: Battery,
        profiles: PowerProfiles,
    ) -> Policy {
        let [dim, screen_off, suspend] = idles;
        let mut policy = Policy {
            chosen: Chosen {
                screen_off: Minutes {
                    plugged: SCREEN_OFF_PLUGGED_DEFAULT,
                    battery: SCREEN_OFF_BATTERY_DEFAULT,
                },
                suspend: Minutes {
                    plugged: SUSPEND_PLUGGED_DEFAULT,
                    battery: SUSPEND_BATTERY_DEFAULT,
                },
                dim: DIM_DEFAULT,
                saver_on_low_battery: SAVER_ON_LOW_BATTERY_DEFAULT,
            },
            on_battery: false,
            undimmed: None,
            saver_replaced: None,
            timers: Timers {
                dim,
                screen_off,
                suspend,
            },
            power,
            battery,
            profiles,
        };
        policy.review();
        policy
    }

    pub(super) fn set_screen_off(&mut self, change: impl FnOnce(&mut Minutes)) {
        change(&mut self.chosen.screen_off);
        self.arm();
    }

    pub(super) fn set_suspend(&mut self, change: impl FnOnce(&mut Minutes)) {
        change(&mut self.chosen.suspend);
        self.arm();
    }

    pub(super) fn set_dim(&mut self, dim: bool) {
        self.chosen.dim = dim;
        self.arm();
    }

    pub(super) fn set_saver_on_low_battery(&mut self, on: bool) {
        self.chosen.saver_on_low_battery = on;
        self.review();
    }

    pub(super) fn review(&mut self) {
        let state = self.battery.state();
        let moved = self.on_battery != state.on_battery;
        self.on_battery = state.on_battery;
        if moved {
            self.arm();
        }
        let low = state.on_battery
            && state
                .main
                .is_some_and(|reading| reading.warning >= Warning::Low);
        self.save_power(low);
    }

    fn save_power(&mut self, low: bool) {
        let profiles = self.profiles.state();
        let Some(active) = profiles.active else {
            return;
        };
        let wanted = low && self.chosen.saver_on_low_battery;
        match self.saver_replaced {
            Some(_) if active != Profile::PowerSaver => self.saver_replaced = None,
            Some(previous) if !wanted => {
                self.saver_replaced = None;
                tracing::info!(
                    "battery recovered; returning to the {} profile",
                    previous.name()
                );
                self.profiles.set(previous);
            }
            Some(_) => {}
            None if wanted
                && active != Profile::PowerSaver
                && profiles.profiles.contains(&Profile::PowerSaver) =>
            {
                self.saver_replaced = Some(active);
                tracing::info!("battery low; saving power");
                self.profiles.set(Profile::PowerSaver);
            }
            None => {}
        }
    }

    fn arm(&self) {
        let pick = |minutes: Minutes| match self.on_battery {
            true => minutes.battery,
            false => minutes.plugged,
        };
        let screen_off = idle_minutes(pick(self.chosen.screen_off));
        let dim = screen_off
            .filter(|_| self.chosen.dim)
            .map(|off| off.saturating_sub(DIM_LEAD).max(DIM_SHORTEST))
            .filter(|dim| screen_off.is_some_and(|off| *dim < off));
        self.timers.dim.after(dim);
        self.timers.screen_off.after(screen_off);
        self.timers
            .suspend
            .after(idle_minutes(pick(self.chosen.suspend)));
    }

    fn dim(&mut self) {
        let Some(level) = self.power.state().brightness else {
            return;
        };
        self.undimmed = Some(level);
        self.power.brighten(level * DIMMED);
    }

    fn undim(&mut self) {
        if let Some(level) = self.undimmed.take() {
            self.power.brighten(level);
        }
    }

    fn suspend(&self) {
        if self.power.state().capabilities.allows(Action::Suspend) {
            tracing::info!("idle; suspending");
            self.power.ask(Action::Suspend);
        }
    }
}

pub(super) fn listen(idles: &[Idle; 3], held: Slot<Policy>) {
    let [dim, screen_off, suspend] = idles;
    dim.on_idle(move |host| host.get_mut(held).dim());
    dim.on_resume(move |host| host.get_mut(held).undim());
    screen_off.on_idle(|host| display_power::sleep(host.seam()));
    suspend.on_idle(move |host| host.get(held).suspend());
}
