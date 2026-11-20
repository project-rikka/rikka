use std::time::Duration;

use desk_layout::Meter;
use rikka_format::format_bytes;
use rikka_sysmon::Vitals;
use rikka_ui::{Label, tr};
use shell_feeds::percent;

use crate::widgets::monitor::history::Trace;

const DOWN: char = '\u{2193}';
const UP: char = '\u{2191}';
const KIBI: f32 = 1024.0;
const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];

pub(super) struct MeterReading {
    pub(crate) meter: Meter,
    pub(crate) level: Option<f32>,
    pub(crate) shown: Label,
    brief: Option<Label>,
    pub(crate) detail: Option<Label>,
}

impl MeterReading {
    #[must_use]
    pub(crate) fn label(&self) -> Label {
        named(self.meter)
    }

    #[must_use]
    pub(crate) fn short(&self) -> Label {
        short(self.meter)
    }

    #[must_use]
    pub(super) fn brief(&self) -> Label {
        self.brief.clone().unwrap_or_else(|| self.shown.clone())
    }

    #[must_use]
    pub(crate) fn trace(&self) -> Trace {
        match self.meter {
            Meter::Cpu => Trace::Cpu,
            Meter::Memory => Trace::Memory,
            Meter::Gpu => Trace::Gpu,
            Meter::Network => Trace::Receive,
            Meter::Disk => Trace::Io,
        }
    }
}

#[must_use]
pub(crate) fn gauged(meter: Meter, vitals: &Vitals) -> Option<MeterReading> {
    let gauged = |level: Option<f32>, shown: String, detail: Option<Label>| MeterReading {
        meter,
        level,
        shown: Label::from(shown),
        brief: None,
        detail,
    };
    Some(match meter {
        Meter::Cpu => {
            let details: Vec<String> = [
                (vitals.cpu.ghz > 0.0).then(|| format!("{:.1} GHz", vitals.cpu.ghz)),
                vitals.cpu_temperature().map(celsius),
            ]
            .into_iter()
            .flatten()
            .collect();
            gauged(
                Some(vitals.cpu.usage),
                percent(vitals.cpu.usage),
                (!details.is_empty()).then(|| Label::from(details.join(" · "))),
            )
        }
        Meter::Memory => {
            let memory = &vitals.memory;
            let used = format_bytes(memory.used);
            let total = format_bytes(memory.total);
            let detail = tr!(
                "shell.desk.widget.monitor.of",
                used = used.as_str(),
                total = total.as_str(),
            );
            MeterReading {
                brief: Some(Label::from(percent(memory.fraction()))),
                ..gauged(Some(memory.fraction()), used, Some(detail))
            }
        }
        Meter::Gpu => {
            let gpu = vitals.main_gpu()?;
            let mut details = vec![gpu.name.clone()];
            details.extend(gpu.temperature.map(celsius));
            gauged(
                Some(gpu.usage),
                percent(gpu.usage),
                Some(Label::from(details.join(" · "))),
            )
        }
        Meter::Network => {
            let traffic = vitals.traffic();
            MeterReading {
                brief: Some(Label::from(format!("{DOWN}{}", compact(traffic.receive)))),
                ..gauged(
                    None,
                    format!("{DOWN} {}/s", compact(traffic.receive)),
                    Some(Label::from(format!("{UP} {}", rate(traffic.send)))),
                )
            }
        }
        Meter::Disk => {
            let volume = vitals.system_volume()?;
            let free = format_bytes(volume.available);
            let io = vitals.io();
            let detail = tr!(
                "shell.desk.widget.monitor.free",
                free = free.as_str(),
                read = rate(io.read).as_str(),
                write = rate(io.write).as_str(),
            );
            gauged(
                Some(volume.fraction()),
                percent(volume.fraction()),
                Some(detail),
            )
        }
    })
}

fn celsius(degrees: f32) -> String {
    format!("{degrees:.0} °C")
}

#[must_use]
pub(crate) fn named(meter: Meter) -> Label {
    match meter {
        Meter::Cpu => tr!("shell.desk.widget.monitor.cpu"),
        Meter::Memory => tr!("shell.desk.widget.monitor.memory"),
        Meter::Gpu => tr!("shell.desk.widget.monitor.gpu"),
        Meter::Network => tr!("shell.desk.widget.monitor.network"),
        Meter::Disk => tr!("shell.desk.widget.monitor.disk"),
    }
}

fn short(meter: Meter) -> Label {
    match meter {
        Meter::Cpu => tr!("shell.desk.widget.monitor.short.cpu"),
        Meter::Memory => tr!("shell.desk.widget.monitor.short.memory"),
        Meter::Gpu => tr!("shell.desk.widget.monitor.short.gpu"),
        Meter::Network => tr!("shell.desk.widget.monitor.short.network"),
        Meter::Disk => tr!("shell.desk.widget.monitor.short.disk"),
    }
}

#[must_use]
pub(crate) fn rate(per_second: f32) -> String {
    format!("{}/s", format_bytes(per_second.max(0.0) as u64))
}

fn compact(per_second: f32) -> String {
    let mut value = per_second.max(0.0);
    let mut unit = 0;
    while value >= KIBI && unit + 1 < UNITS.len() {
        value /= KIBI;
        unit += 1;
    }
    match value < 10.0 && unit > 0 {
        true => format!("{value:.1}{}", UNITS[unit]),
        false => format!("{value:.0}{}", UNITS[unit]),
    }
}

#[must_use]
pub(super) fn uptime(up: Duration) -> Label {
    let minutes = up.as_secs() / 60;
    let (days, hours, minutes) = (minutes / 1440, minutes / 60 % 24, minutes % 60);
    match days {
        0 => tr!(
            "shell.desk.widget.monitor.up-hours",
            hours = hours,
            minutes = minutes,
        ),
        _ => tr!(
            "shell.desk.widget.monitor.up-days",
            days = days,
            hours = hours,
        ),
    }
}
