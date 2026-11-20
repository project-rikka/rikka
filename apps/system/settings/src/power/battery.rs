use std::time::Duration;

use rikka_app::prelude::*;
use rikka_battery::{Charge, Device, DeviceKind, Level, Pack, Reading, Technology};
use rikka_settings::session::power::BATTERY_PERCENTAGE;
use rikka_settings::session::power::BATTERY_PERCENTAGE_DEFAULT;
use rikka_widgets::{column, row};

use crate::controls::flag_or;
use crate::info_row::info_row;
use crate::power::chart::Chart;
use crate::power::component::{HISTORY_SPAN, PowerPage};

const CHART: f32 = 96.0;
const HOUR: u64 = 3600;
const MINUTE: u64 = 60;

pub(crate) fn view(page: &PowerPage, reading: Reading) -> impl View {
    section(
        tr!("settings.power.battery"),
        column! {
            row! {
                icon(symbol(&reading)).extent(Length::px(36.0));
                column! {
                    text(percent(reading.charge)).title().semibold();
                    text(condition(&reading)).label().muted();
                }
                .gap(Space::TIGHT);
                spacer().grow(1.0);
                reading.rate.map(|watts| {
                    text(tr!("settings.power.rate", watts = format!("{watts:.1}").as_str()))
                        .label()
                        .muted()
                });
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width();
            history(page);
            for pack in &page.battery().packs {
                details(pack);
            }
            flag_or(
                page,
                tr!("settings.power.percentage"),
                BATTERY_PERCENTAGE,
                BATTERY_PERCENTAGE_DEFAULT,
            );
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}

fn history(page: &PowerPage) -> impl View {
    column! {
        row! {
            text(tr!("settings.power.history")).caption().muted();
            spacer().grow(1.0);
            text(tr!("settings.power.history-now")).caption().muted();
        }
        .fill_width();
        Element::new(Chart {
            samples: page.history(),
            span: HISTORY_SPAN,
            until: page.read_at(),
        })
        .height(CHART)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn details(pack: &Pack) -> impl View {
    let id = pack.id.clone();
    column! {
        info_row(tr!("settings.power.model"), pack.name.clone());
        info_row(
            tr!("settings.power.health"),
            pack.health.map(percent).unwrap_or_default(),
        );
        info_row(tr!("settings.power.capacity"), capacity(pack));
        info_row(
            tr!("settings.power.cycles"),
            pack.cycles.map(|cycles| cycles.to_string()).unwrap_or_default(),
        );
        info_row(tr!("settings.power.chemistry"), technology(pack.technology));
        pack.threshold.map(|threshold| {
            column! {
                field(
                    tr!("settings.power.preserve"),
                    switch(threshold.enabled).on_toggle(move |page: &mut PowerPage, on: bool| {
                        page.preserve(id.clone(), on);
                    }),
                );
                text(tr!(
                    "settings.power.preserve-hint",
                    start = threshold.start,
                    end = threshold.end,
                ))
                .caption()
                .muted();
            }
            .gap(Space::TIGHT)
            .fill_width()
        });
    }
    .gap(Space::SNUG)
    .fill_width()
}

pub(crate) fn peripherals(devices: &[Device]) -> impl View {
    section(
        tr!("settings.power.devices"),
        column! {
            for device in devices {
                row! {
                    icon(kind_symbol(device.kind)).extent(Length::px(20.0));
                    text(match device.name.is_empty() {
                        true => kind_label(device.kind),
                        false => Label::from(device.name.as_str()),
                    })
                    .label()
                    .no_wrap();
                    spacer().grow(1.0);
                    text(device_charge(device)).label().muted();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

pub(crate) fn percent(fraction: f32) -> String {
    format!("{}%", (fraction.clamp(0.0, 1.0) * 100.0).round() as u8)
}

fn condition(reading: &Reading) -> Label {
    let left = reading.left.map(span);
    match (reading.state, left) {
        (Charge::Full, _) => tr!("settings.power.full"),
        (Charge::Charging, Some(left)) => {
            tr!("settings.power.charging-until", left = left.as_str())
        }
        (Charge::Charging, None) => tr!("settings.power.charging"),
        (Charge::Discharging, Some(left)) => tr!("settings.power.left", left = left.as_str()),
        (Charge::Discharging, None) => tr!("settings.power.discharging"),
        (Charge::Empty, _) => tr!("settings.power.empty"),
        (Charge::Unknown, _) => tr!("settings.power.not-charging"),
    }
}

pub(crate) fn span(left: Duration) -> String {
    let seconds = left.as_secs();
    let (hours, minutes) = (seconds / HOUR, seconds % HOUR / MINUTE);
    match hours {
        0 => tr!("settings.power.span-minutes", minutes = minutes).to_string(),
        _ => tr!("settings.power.span", hours = hours, minutes = minutes).to_string(),
    }
}

fn capacity(pack: &Pack) -> String {
    match (pack.energy_full, pack.energy_full_design) {
        (Some(full), Some(design)) => tr!(
            "settings.power.capacity-reading",
            full = format!("{full:.1}").as_str(),
            design = format!("{design:.1}").as_str(),
        )
        .to_string(),
        _ => String::new(),
    }
}

fn technology(technology: Technology) -> String {
    match technology {
        Technology::Unknown => String::new(),
        Technology::LithiumIon => tr!("settings.power.technology.lithium-ion").to_string(),
        Technology::LithiumPolymer => tr!("settings.power.technology.lithium-polymer").to_string(),
        Technology::LithiumIronPhosphate => {
            tr!("settings.power.technology.lithium-iron-phosphate").to_string()
        }
        Technology::LeadAcid => tr!("settings.power.technology.lead-acid").to_string(),
        Technology::NickelCadmium => tr!("settings.power.technology.nickel-cadmium").to_string(),
        Technology::NickelMetalHydride => {
            tr!("settings.power.technology.nickel-metal-hydride").to_string()
        }
    }
}

fn symbol(reading: &Reading) -> MaterialSymbol {
    match reading.state {
        Charge::Charging => MaterialSymbol::BATTERY_CHARGING_FULL,
        Charge::Full => MaterialSymbol::BATTERY_FULL,
        _ if reading.charge <= 0.1 => MaterialSymbol::BATTERY_ALERT,
        _ => match (reading.charge * 6.0).round() as u8 {
            0 => MaterialSymbol::BATTERY_0_BAR,
            1 => MaterialSymbol::BATTERY_1_BAR,
            2 => MaterialSymbol::BATTERY_2_BAR,
            3 => MaterialSymbol::BATTERY_3_BAR,
            4 => MaterialSymbol::BATTERY_4_BAR,
            5 => MaterialSymbol::BATTERY_5_BAR,
            _ => MaterialSymbol::BATTERY_6_BAR,
        },
    }
}

fn device_charge(device: &Device) -> Label {
    if let Some(charge) = device.charge {
        return Label::from(percent(charge).as_str());
    }
    match device.level {
        Level::Unknown => tr!("settings.power.level-unknown"),
        Level::Critical => tr!("settings.power.level-critical"),
        Level::Low => tr!("settings.power.level-low"),
        Level::Normal => tr!("settings.power.level-normal"),
        Level::High => tr!("settings.power.level-high"),
        Level::Full => tr!("settings.power.level-full"),
    }
}

fn kind_symbol(kind: DeviceKind) -> MaterialSymbol {
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

fn kind_label(kind: DeviceKind) -> Label {
    match kind {
        DeviceKind::Mouse => tr!("settings.power.kind.mouse"),
        DeviceKind::Keyboard => tr!("settings.power.kind.keyboard"),
        DeviceKind::Touchpad => tr!("settings.power.kind.touchpad"),
        DeviceKind::Pen => tr!("settings.power.kind.pen"),
        DeviceKind::Tablet => tr!("settings.power.kind.tablet"),
        DeviceKind::Phone => tr!("settings.power.kind.phone"),
        DeviceKind::Headset => tr!("settings.power.kind.headset"),
        DeviceKind::Headphones => tr!("settings.power.kind.headphones"),
        DeviceKind::Speakers => tr!("settings.power.kind.speakers"),
        DeviceKind::GamingInput => tr!("settings.power.kind.gaming-input"),
        DeviceKind::Wearable => tr!("settings.power.kind.wearable"),
        DeviceKind::RemoteControl => tr!("settings.power.kind.remote-control"),
        DeviceKind::MediaPlayer => tr!("settings.power.kind.media-player"),
        DeviceKind::Computer => tr!("settings.power.kind.computer"),
        DeviceKind::Ups => tr!("settings.power.kind.ups"),
        DeviceKind::Other => tr!("settings.power.kind.other"),
    }
}
