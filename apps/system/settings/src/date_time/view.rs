use jiff::Timestamp;
use jiff::tz::TimeZone;
use rikka_app::prelude::*;
use rikka_nix::Module;
use rikka_widgets::{column, row};

use crate::date_time::component::{
    self, DateTimePage, Part, TIME_ZONE, keeps_local_rtc, locates, synchronizes,
};
use crate::date_time::spoken;
use crate::date_time::zones::Zone;
use crate::notice::notice;
use crate::pending::Pending;
use crate::settings::Settings;

const SERVERS: f32 = 320.0;
const ZONES: f32 = 320.0;
const YEAR: f32 = 88.0;
const PART: f32 = 64.0;

pub(crate) fn view(page: &DateTimePage, pending: &Pending) -> impl View {
    column! {
        page.state().error.as_ref().map(|refused| {
            notice(
                MaterialSymbol::ERROR,
                palette().danger,
                tr!("settings.date-time.refused", reason = refused.reason.as_str()),
            )
        });
        component_with(
            |page: &DateTimePage| page,
            shown_zone(page, pending).map(|zone| zone.name),
            now,
        );
        time(page, pending);
        zone(page, pending);
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn now(page: &DateTimePage, zone: &Option<String>) -> impl View {
    let at = page.ticking().now();
    let local = at.to_zoned(TimeZone::system());
    let zone = zone
        .as_deref()
        .and_then(|name| page.zone(name))
        .map(|zone| {
            tr!(
                "settings.date-time.in-zone",
                zone = zone.name.as_str(),
                offset = zone.offset(at).as_str()
            )
        });
    card(
        column! {
            text(spoken::clock(&local)).display().semibold();
            text(spoken::long_date(&local)).body();
            zone.map(|zone| text(zone).label().muted());
            text(synced(page.state())).caption().muted();
        }
        .gap(Space::TIGHT)
        .fill_width(),
    )
}

fn synced(state: &rikka_timedate::State) -> Label {
    match (state.ntp, state.synchronized, &state.server) {
        (false, ..) => tr!("settings.date-time.sync.off"),
        (true, true, Some(server)) => {
            tr!("settings.date-time.sync.with", server = server.as_str())
        }
        (true, true, None) => tr!("settings.date-time.sync.done"),
        (true, false, _) => tr!("settings.date-time.sync.waiting"),
    }
}

fn time(page: &DateTimePage, pending: &Pending) -> impl View {
    let automatic = synchronizes(pending);
    let typed = page.servers().text();
    section(
        tr!("settings.date-time.time"),
        column! {
            field(
                tr!("settings.date-time.automatic"),
                switch(automatic).on_toggle(component::set_synchronized),
            );
            if automatic {
                field(
                    tr!("settings.date-time.servers"),
                    text_field(page.servers())
                        .placeholder(tr!("settings.date-time.servers-default"))
                        .error(!component::servers_typed_well(&typed))
                        .on_change(|| {})
                        .on_blur(|settings: &mut Settings| settings.commit_time_servers())
                        .on_submit(|settings: &mut Settings| settings.commit_time_servers())
                        .width(SERVERS),
                );
            }
            manual(page);
            field(
                tr!("settings.date-time.local-rtc"),
                switch(keeps_local_rtc(pending)).on_toggle(component::set_local_rtc),
            );
            text(tr!("settings.date-time.local-rtc-hint")).caption().muted();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn manual(page: &DateTimePage) -> impl View {
    let settable = !page.state().ntp;
    let at = page.manual().at;
    let part = |part: Part, value: i64, range: std::ops::RangeInclusive<f32>, width: f32| {
        number_field(value as f32)
            .range(range)
            .step(1.0)
            .enabled(settable)
            .control_size(ControlSize::Small)
            .on_commit(move |page: &mut DateTimePage, moved: f32| page.adjust(part, moved))
            .width(width)
    };
    column! {
        field(
            tr!("settings.date-time.manual"),
            row! {
                part(Part::Year, i64::from(at.year()), 1970.0..=9999.0, YEAR);
                text("-").label().muted();
                part(Part::Month, i64::from(at.month()), 1.0..=12.0, PART);
                text("-").label().muted();
                part(Part::Day, i64::from(at.day()), 1.0..=31.0, PART);
                spacer().width(8.0);
                part(Part::Hour, i64::from(at.hour()), 0.0..=23.0, PART);
                text(":").label().muted();
                part(Part::Minute, i64::from(at.minute()), 0.0..=59.0, PART);
                button(tr!("settings.date-time.set"))
                    .small()
                    .enabled(settable)
                    .on_press(DateTimePage::set_clock);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .dimmed(!settable);
        if !settable {
            text(tr!("settings.date-time.manual-hint")).caption().muted();
        }
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn zone(page: &DateTimePage, pending: &Pending) -> impl View {
    let automatic = locates(pending);
    let shown = shown_zone(page, pending);
    let current = page.state().zone.clone();
    section(
        tr!("settings.date-time.zone"),
        column! {
            field(
                tr!("settings.date-time.automatic-zone"),
                switch(automatic).on_toggle(move |pending: &mut Pending, on: bool| {
                    component::set_locating(pending, on, current.clone());
                }),
            );
            field(
                tr!("settings.date-time.zone-field"),
                row! {
                    text(match &shown {
                        Some(zone) => Label::from(zone_line(zone).as_str()),
                        None => tr!("settings.date-time.zone-unset"),
                    })
                    .label()
                    .no_wrap();
                    button(tr!("settings.date-time.change"))
                        .secondary()
                        .small()
                        .enabled(!automatic && page.choosing().is_none())
                        .on_press(DateTimePage::open_chooser);
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .dimmed(automatic);
            if automatic {
                text(tr!("settings.date-time.automatic-zone-hint")).caption().muted();
            }
            if let Some(typed) = page.choosing().filter(|_| !automatic) {
                chooser(page, typed);
            }
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn chooser(page: &DateTimePage, typed: &TextDocument) -> impl View {
    let offered = page.offered();
    let now = Timestamp::now();
    column! {
        row! {
            text_field(typed)
                .placeholder(tr!("settings.date-time.zone-search"))
                .on_change(|| {})
                .fill_width();
            button(icon(MaterialSymbol::CLOSE))
                .ghost()
                .small()
                .icon_only()
                .on_press(DateTimePage::close_chooser);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width();
        if offered.is_empty() {
            text(tr!("settings.date-time.zone-nothing")).label().muted();
        } else {
            list(offered.iter().map(|zone| {
                let place = match &zone.country {
                    Some(country) => format!("{country} · {}", zone.name),
                    None => zone.name.clone(),
                };
                list_item(zone.city.as_str())
                    .secondary(place)
                    .trailing(format!("{} · {}", zone.offset(now), zone.clock(now)))
                    .keyed(zone.name.as_str())
            }))
            .selected(None)
            .control_size(ControlSize::Small)
            .on_select(|settings: &mut Settings, index: usize| settings.choose_zone(index))
            .height(ZONES)
            .fill_width();
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn shown_zone(page: &DateTimePage, pending: &Pending) -> Option<Zone> {
    let staged = pending.get::<String>(Module::System, TIME_ZONE);
    let named = match locates(pending) {
        true => page.state().zone.clone(),
        false => staged.or_else(|| page.state().zone.clone()),
    };
    page.zone(&named?)
}

fn zone_line(zone: &Zone) -> String {
    format!(
        "{} ({})",
        zone.name.replace('_', " "),
        zone.offset(Timestamp::now())
    )
}
