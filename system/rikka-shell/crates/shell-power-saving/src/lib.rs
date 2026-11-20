mod policy;
mod service;
mod watch;

use policy::Policy;
use rikka_battery::Battery;
use rikka_power::Power;
use rikka_power_profiles::PowerProfiles;
use rikka_settings::session::power::{
    DIM, DOMAIN, SAVER_ON_LOW_BATTERY, SCREEN_OFF_BATTERY, SCREEN_OFF_PLUGGED, SUSPEND_BATTERY,
    SUSPEND_PLUGGED, schema,
};
use service::PowerSavingService;
use shell_kit::{Feature, Shell};

#[derive(Debug)]
pub struct PowerSaving {
    power: Power,
    battery: Battery,
    profiles: PowerProfiles,
}

impl PowerSaving {
    #[must_use]
    pub fn new(power: Power, battery: Battery, profiles: PowerProfiles) -> PowerSaving {
        PowerSaving {
            power,
            battery,
            profiles,
        }
    }
}

impl Feature for PowerSaving {
    fn install(self, shell: Shell) -> Shell {
        let shell = shell.settings(schema());
        watch::watch(&self.battery, &self.profiles, shell.seam());
        let idles = [shell.idle(), shell.idle(), shell.idle()];
        let policy = Policy::new(idles.clone(), self.power, self.battery, self.profiles);
        let (shell, held) = shell.hold(policy);
        policy::listen(&idles, held);
        let shell = shell.service(PowerSavingService::new(held));
        shell
            .follow(DOMAIN, SCREEN_OFF_PLUGGED, move |host, minutes: i64| {
                host.get_mut(held)
                    .set_screen_off(|chosen| chosen.plugged = minutes);
            })
            .follow(DOMAIN, SCREEN_OFF_BATTERY, move |host, minutes: i64| {
                host.get_mut(held)
                    .set_screen_off(|chosen| chosen.battery = minutes);
            })
            .follow(DOMAIN, SUSPEND_PLUGGED, move |host, minutes: i64| {
                host.get_mut(held)
                    .set_suspend(|chosen| chosen.plugged = minutes);
            })
            .follow(DOMAIN, SUSPEND_BATTERY, move |host, minutes: i64| {
                host.get_mut(held)
                    .set_suspend(|chosen| chosen.battery = minutes);
            })
            .follow(DOMAIN, DIM, move |host, dim: bool| {
                host.get_mut(held).set_dim(dim)
            })
            .follow(DOMAIN, SAVER_ON_LOW_BATTERY, move |host, on: bool| {
                host.get_mut(held).set_saver_on_low_battery(on);
            })
    }
}
