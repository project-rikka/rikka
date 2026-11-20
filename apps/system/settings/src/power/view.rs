use rikka_app::prelude::*;
use rikka_power_profiles::{Degraded, Profile};
use rikka_settings::session::power::{
    DIM, DIM_DEFAULT, MINUTES, SAVER_ON_LOW_BATTERY, SAVER_ON_LOW_BATTERY_DEFAULT,
    SCREEN_OFF_BATTERY, SCREEN_OFF_BATTERY_DEFAULT, SCREEN_OFF_PLUGGED, SCREEN_OFF_PLUGGED_DEFAULT,
    SUSPEND_BATTERY, SUSPEND_BATTERY_DEFAULT, SUSPEND_PLUGGED, SUSPEND_PLUGGED_DEFAULT,
};
use rikka_widgets::column;

use crate::amount::amount;
use crate::controls::{Count, flag_or, span};
use crate::domain::DomainPage;
use crate::notice::{domain_notice, notice};
use crate::pending::Pending;
use crate::picker::picker;
use crate::power::battery;
use crate::power::buttons::{self, Response, Trigger};
use crate::power::component::PowerPage;
use crate::power::critical::{self, Critical, Level};

const PROFILES: [Profile; 3] = [Profile::PowerSaver, Profile::Balanced, Profile::Performance];

pub(crate) fn view(page: &PowerPage, pending: &Pending) -> impl View {
    let reading = page.battery().main;
    let reasons = [
        page.power()
            .error
            .as_ref()
            .map(|refused| refused.reason.clone()),
        page.battery()
            .error
            .as_ref()
            .map(|refused| refused.reason.clone()),
        page.profiles()
            .error
            .as_ref()
            .map(|refused| refused.reason.clone()),
    ];
    column! {
        domain_notice(page.domain(), "settings.power.unavailable", "settings.power.refused");
        for reason in reasons.into_iter().flatten() {
            notice(
                MaterialSymbol::ERROR,
                palette().danger,
                tr!("settings.power.failed", reason = reason.as_str()),
            );
        }
        reading.map(|reading| battery::view(page, reading));
        if !page.battery().devices.is_empty() {
            battery::peripherals(&page.battery().devices);
        }
        mode(page, reading.is_some());
        screen(page, reading.is_some());
        sleep(page, reading.is_some());
        buttons(page, pending);
        if reading.is_some() {
            low_battery(page, pending);
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn mode(page: &PowerPage, has_battery: bool) -> Option<impl View> {
    let profiles = page.profiles();
    let active = profiles.active?;
    let offered: Vec<Profile> = PROFILES
        .into_iter()
        .filter(|profile| profiles.profiles.contains(profile))
        .collect();
    let selected = offered.iter().position(|profile| *profile == active);
    let chosen = offered.clone();
    Some(section(
        tr!("settings.power.mode"),
        column! {
            segmented(selected.unwrap_or_default())
                .segments(offered.iter().map(|profile| profile_label(*profile)))
                .on_select(move |page: &mut PowerPage, index: usize| {
                    if let Some(profile) = chosen.get(index) {
                        page.choose_profile(*profile);
                    }
                })
                .fill_width();
            text(profile_hint(active)).caption().muted();
            profiles.degraded.as_ref().map(|degraded| {
                notice(
                    MaterialSymbol::THERMOSTAT,
                    palette().warning,
                    degraded_reason(degraded),
                )
            });
            for hold in &profiles.holds {
                text(tr!(
                    "settings.power.hold",
                    app = hold.application.as_str(),
                    profile = profile_label(hold.profile).to_string().as_str(),
                    reason = hold.reason.as_str(),
                ))
                .caption()
                .muted();
            }
            if has_battery {
                flag_or(
                    page,
                    tr!("settings.power.saver-on-low-battery"),
                    SAVER_ON_LOW_BATTERY,
                    SAVER_ON_LOW_BATTERY_DEFAULT,
                );
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    ))
}

fn screen(page: &PowerPage, has_battery: bool) -> impl View {
    let minutes = span(&MINUTES);
    section(
        tr!("settings.power.screen"),
        column! {
            page.power().brightness.map(|level| {
                let percent = (level * 100.0).round();
                field(
                    tr!("settings.power.brightness"),
                    amount(
                        percent,
                        1.0..=100.0,
                        1.0,
                        format!("{percent:.0}%"),
                        PowerPage::brighten,
                    ),
                )
            });
            Count {
                key: SCREEN_OFF_PLUGGED,
                absent: SCREEN_OFF_PLUGGED_DEFAULT,
                range: minutes.clone(),
            }
            .field(
                page,
                match has_battery {
                    true => tr!("settings.power.screen-off-plugged"),
                    false => tr!("settings.power.screen-off"),
                },
                spoken_minutes,
            );
            if has_battery {
                Count {
                    key: SCREEN_OFF_BATTERY,
                    absent: SCREEN_OFF_BATTERY_DEFAULT,
                    range: minutes.clone(),
                }
                .field(page, tr!("settings.power.screen-off-battery"), spoken_minutes);
            }
            flag_or(page, tr!("settings.power.dim"), DIM, DIM_DEFAULT);
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn sleep(page: &PowerPage, has_battery: bool) -> impl View {
    let minutes = span(&MINUTES);
    let suspends = page
        .power()
        .capabilities
        .allows(rikka_power::Action::Suspend);
    section(
        tr!("settings.power.sleep"),
        column! {
            Count {
                key: SUSPEND_PLUGGED,
                absent: SUSPEND_PLUGGED_DEFAULT,
                range: minutes.clone(),
            }
            .field(
                page,
                match has_battery {
                    true => tr!("settings.power.suspend-plugged"),
                    false => tr!("settings.power.suspend"),
                },
                spoken_minutes,
            )
            .dimmed(!suspends);
            if has_battery {
                Count {
                    key: SUSPEND_BATTERY,
                    absent: SUSPEND_BATTERY_DEFAULT,
                    range: minutes.clone(),
                }
                .field(page, tr!("settings.power.suspend-battery"), spoken_minutes)
                .dimmed(!suspends);
            }
            if !suspends {
                text(tr!("settings.power.suspend-unavailable")).caption().muted();
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn buttons(page: &PowerPage, pending: &Pending) -> impl View {
    let offered = buttons::offered(page.power().capabilities.hibernates());
    let triggers: &[Trigger] = match page.has_lid() {
        true => &[
            Trigger::PowerKey,
            Trigger::Lid,
            Trigger::LidPlugged,
            Trigger::LidDocked,
        ],
        false => &[Trigger::PowerKey],
    };
    section(
        tr!("settings.power.buttons"),
        column! {
            for trigger in triggers {
                response_field(pending, *trigger, &offered);
            }
            text(tr!("settings.power.buttons-hint")).caption().muted();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn response_field(pending: &Pending, trigger: Trigger, offered: &[Response]) -> impl View {
    let current = buttons::response(pending, trigger);
    let choices = offered.to_vec();
    field(
        trigger.label(),
        picker(
            offered.iter().position(|response| *response == current),
            offered.iter().map(|response| response.label()),
            move |pending: &mut Pending, index: usize| {
                if let Some(chosen) = choices.get(index) {
                    buttons::respond(pending, trigger, *chosen);
                }
            },
        ),
    )
}

fn low_battery(page: &PowerPage, pending: &Pending) -> impl View {
    let offered = Critical::offered(page.power().capabilities.hibernates());
    let current = critical::action(pending);
    let choices = offered.clone();
    section(
        tr!("settings.power.low-battery"),
        column! {
            for which in [Level::Low, Level::Critical, Level::Acting] {
                level_field(pending, which);
            }
            field(
                tr!("settings.power.critical-action"),
                picker(
                    offered.iter().position(|action| *action == current),
                    offered.iter().map(|action| action.label()),
                    move |pending: &mut Pending, index: usize| {
                        if let Some(chosen) = choices.get(index) {
                            critical::set_action(pending, *chosen);
                        }
                    },
                ),
            );
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn level_field(pending: &Pending, which: Level) -> impl View {
    let value = critical::level(pending, which);
    let range = critical::range(pending, which);
    field(
        which.label(),
        amount(
            value as f32,
            *range.start() as f32..=*range.end() as f32,
            1.0,
            format!("{value}%"),
            move |pending: &mut Pending, moved: f32| {
                critical::set_level(pending, which, moved.round() as i64);
            },
        ),
    )
}

fn spoken_minutes(minutes: i64) -> String {
    match minutes {
        0 => tr!("settings.power.never").to_string(),
        minutes => tr!("settings.power.minutes", count = minutes).to_string(),
    }
}

fn profile_label(profile: Profile) -> Label {
    match profile {
        Profile::PowerSaver => tr!("settings.power.profile.power-saver"),
        Profile::Balanced => tr!("settings.power.profile.balanced"),
        Profile::Performance => tr!("settings.power.profile.performance"),
    }
}

fn profile_hint(profile: Profile) -> Label {
    match profile {
        Profile::PowerSaver => tr!("settings.power.profile.power-saver-hint"),
        Profile::Balanced => tr!("settings.power.profile.balanced-hint"),
        Profile::Performance => tr!("settings.power.profile.performance-hint"),
    }
}

fn degraded_reason(degraded: &Degraded) -> Label {
    match degraded {
        Degraded::LapDetected => tr!("settings.power.degraded.lap"),
        Degraded::HighTemperature => tr!("settings.power.degraded.temperature"),
        Degraded::Other(reason) => tr!("settings.power.degraded.other", reason = reason.as_str()),
    }
}
