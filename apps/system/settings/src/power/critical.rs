use std::ops::RangeInclusive;

use rikka_app::prelude::*;
use rikka_nix::Module;

use crate::pending::Pending;

const ACTION: &str = "services.upower.criticalPowerAction";
const RISKY: &str = "services.upower.allowRiskyCriticalPowerAction";
const LOW: &str = "services.upower.percentageLow";
const CRITICAL: &str = "services.upower.percentageCritical";
const ACTING: &str = "services.upower.percentageAction";

const LOW_DEFAULT: i64 = 20;
const CRITICAL_DEFAULT: i64 = 5;
const ACTING_DEFAULT: i64 = 2;
const HIGHEST_LOW: i64 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Critical {
    HybridSleep,
    Hibernate,
    PowerOff,
    Suspend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    Low,
    Critical,
    Acting,
}

impl Critical {
    const ALL: [Critical; 4] = [
        Critical::HybridSleep,
        Critical::Hibernate,
        Critical::PowerOff,
        Critical::Suspend,
    ];

    fn value(self) -> &'static str {
        match self {
            Critical::HybridSleep => "HybridSleep",
            Critical::Hibernate => "Hibernate",
            Critical::PowerOff => "PowerOff",
            Critical::Suspend => "Suspend",
        }
    }

    pub(crate) fn label(self) -> Label {
        match self {
            Critical::HybridSleep => tr!("settings.power.critical.hybrid-sleep"),
            Critical::Hibernate => tr!("settings.power.critical.hibernate"),
            Critical::PowerOff => tr!("settings.power.critical.power-off"),
            Critical::Suspend => tr!("settings.power.critical.suspend"),
        }
    }

    pub(crate) fn offered(hibernates: bool) -> Vec<Critical> {
        Critical::ALL
            .into_iter()
            .filter(|action| {
                hibernates || !matches!(action, Critical::HybridSleep | Critical::Hibernate)
            })
            .collect()
    }
}

impl Level {
    pub(crate) fn label(self) -> Label {
        match self {
            Level::Low => tr!("settings.power.level.low"),
            Level::Critical => tr!("settings.power.level.critical"),
            Level::Acting => tr!("settings.power.level.acting"),
        }
    }

    fn path(self) -> &'static str {
        match self {
            Level::Low => LOW,
            Level::Critical => CRITICAL,
            Level::Acting => ACTING,
        }
    }

    fn fallback(self) -> i64 {
        match self {
            Level::Low => LOW_DEFAULT,
            Level::Critical => CRITICAL_DEFAULT,
            Level::Acting => ACTING_DEFAULT,
        }
    }
}

pub(crate) fn action(pending: &Pending) -> Critical {
    let named = pending.get::<String>(Module::Desktop, ACTION);
    Critical::ALL
        .into_iter()
        .find(|action| named.as_deref() == Some(action.value()))
        .unwrap_or(Critical::HybridSleep)
}

pub(crate) fn set_action(pending: &mut Pending, chosen: Critical) {
    pending.change(Module::Desktop, |document| {
        match chosen {
            Critical::HybridSleep => document.remove(ACTION)?,
            other => document.set(ACTION, other.value())?,
        }
        match chosen {
            Critical::Suspend => document.set(RISKY, true)?,
            _ => document.remove(RISKY)?,
        }
        Ok(())
    });
}

pub(crate) fn level(pending: &Pending, level: Level) -> i64 {
    pending
        .get::<i64>(Module::Desktop, level.path())
        .unwrap_or_else(|| level.fallback())
}

pub(crate) fn range(pending: &Pending, which: Level) -> RangeInclusive<i64> {
    let (lowest, highest) = match which {
        Level::Low => (level(pending, Level::Critical) + 1, HIGHEST_LOW),
        Level::Critical => (
            level(pending, Level::Acting) + 1,
            level(pending, Level::Low) - 1,
        ),
        Level::Acting => (1, level(pending, Level::Critical) - 1),
    };
    lowest.min(highest)..=highest.max(lowest)
}

pub(crate) fn set_level(pending: &mut Pending, which: Level, percent: i64) {
    let allowed = range(pending, which);
    let percent = percent.clamp(*allowed.start(), *allowed.end());
    match percent == which.fallback() {
        true => pending.remove(Module::Desktop, which.path()),
        false => pending.set(Module::Desktop, which.path(), percent),
    }
}
