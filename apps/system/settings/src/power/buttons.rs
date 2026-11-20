use rikka_app::prelude::*;
use rikka_nix::Module;

use crate::pending::Pending;

const LIDS: &str = "/proc/acpi/button/lid";

const LOGIN: &str = "services.logind.settings.Login";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Response {
    Ignore,
    Lock,
    Suspend,
    Hibernate,
    PowerOff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trigger {
    PowerKey,
    Lid,
    LidPlugged,
    LidDocked,
}

impl Response {
    pub(crate) const ALL: [Response; 5] = [
        Response::Ignore,
        Response::Lock,
        Response::Suspend,
        Response::Hibernate,
        Response::PowerOff,
    ];

    fn value(self) -> &'static str {
        match self {
            Response::Ignore => "ignore",
            Response::Lock => "lock",
            Response::Suspend => "suspend",
            Response::Hibernate => "hibernate",
            Response::PowerOff => "poweroff",
        }
    }

    fn named(value: &str) -> Option<Response> {
        Response::ALL
            .into_iter()
            .find(|response| response.value() == value)
    }

    pub(crate) fn label(self) -> Label {
        match self {
            Response::Ignore => tr!("settings.power.response.ignore"),
            Response::Lock => tr!("settings.power.response.lock"),
            Response::Suspend => tr!("settings.power.response.suspend"),
            Response::Hibernate => tr!("settings.power.response.hibernate"),
            Response::PowerOff => tr!("settings.power.response.power-off"),
        }
    }
}

impl Trigger {
    pub(crate) fn label(self) -> Label {
        match self {
            Trigger::PowerKey => tr!("settings.power.power-key"),
            Trigger::Lid => tr!("settings.power.lid"),
            Trigger::LidPlugged => tr!("settings.power.lid-plugged"),
            Trigger::LidDocked => tr!("settings.power.lid-docked"),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Trigger::PowerKey => "HandlePowerKey",
            Trigger::Lid => "HandleLidSwitch",
            Trigger::LidPlugged => "HandleLidSwitchExternalPower",
            Trigger::LidDocked => "HandleLidSwitchDocked",
        }
    }

    fn path(self) -> String {
        format!("{LOGIN}.{}", self.key())
    }

    fn fallback(self, pending: &Pending) -> Response {
        match self {
            Trigger::PowerKey => Response::PowerOff,
            Trigger::Lid => Response::Suspend,
            Trigger::LidPlugged => response(pending, Trigger::Lid),
            Trigger::LidDocked => Response::Ignore,
        }
    }
}

pub(crate) fn has_lid() -> bool {
    std::fs::read_dir(LIDS).is_ok_and(|mut lids| lids.next().is_some())
}

pub(crate) fn response(pending: &Pending, trigger: Trigger) -> Response {
    pending
        .get::<String>(Module::System, &trigger.path())
        .and_then(|value| Response::named(&value))
        .unwrap_or_else(|| trigger.fallback(pending))
}

pub(crate) fn respond(pending: &mut Pending, trigger: Trigger, chosen: Response) {
    let path = trigger.path();
    pending.remove(Module::System, &path);
    if response(pending, trigger) != chosen {
        pending.set(Module::System, &path, chosen.value());
    }
}

pub(crate) fn offered(hibernates: bool) -> Vec<Response> {
    Response::ALL
        .into_iter()
        .filter(|response| hibernates || *response != Response::Hibernate)
        .collect()
}
