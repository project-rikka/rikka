use std::time::Duration;

use rikka_battery::{Charge, Reading};
use rikka_ui::{Label, tr};

#[must_use]
pub(crate) fn condition(reading: &Reading, on_battery: bool) -> Label {
    match (reading.state, reading.left) {
        (Charge::Charging, Some(left)) => {
            let left = spoken(left).to_string();
            tr!("shell.desk.widget.power.until-full", time = left.as_str())
        }
        (Charge::Charging, None) => tr!("shell.desk.widget.power.charging"),
        (Charge::Full, _) => tr!("shell.desk.widget.power.full"),
        (Charge::Discharging, Some(left)) => {
            let left = spoken(left).to_string();
            tr!("shell.desk.widget.power.left", time = left.as_str())
        }
        _ if on_battery => tr!("shell.desk.widget.power.on-battery"),
        _ => tr!("shell.desk.widget.power.connected"),
    }
}

#[must_use]
pub(crate) fn named(name: &str) -> Label {
    match name.trim().is_empty() {
        true => tr!("shell.desk.widget.power.device"),
        false => Label::from(name),
    }
}

fn spoken(left: Duration) -> Label {
    let minutes = left.as_secs().div_ceil(60);
    let (hours, minutes) = (minutes / 60, minutes % 60);
    match hours {
        0 => tr!("shell.desk.widget.power.minutes", minutes = minutes),
        _ => tr!(
            "shell.desk.widget.power.hours",
            hours = hours,
            minutes = minutes,
        ),
    }
}
