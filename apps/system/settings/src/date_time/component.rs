use std::time::SystemTime;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use rikka_app::prelude::*;
use rikka_nix::Module;
use rikka_timedate::TimeDate;

use crate::date_time::ticking::Ticking;
use crate::date_time::zones::{self, Zone};
use crate::host_name;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

pub(crate) const TIME_ZONE: &str = "time.timeZone";
pub(crate) const AUTOMATIC_ZONE: &str = "services.automatic-timezoned.enable";
pub(crate) const SYNCHRONIZED: &str = "services.timesyncd.enable";
pub(crate) const SERVERS: &str = "networking.timeServers";
pub(crate) const LOCAL_RTC: &str = "time.hardwareClockInLocalTime";

const FOLLOWING: &str = "date-time-following";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Year,
    Month,
    Day,
    Hour,
    Minute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Manual {
    pub(crate) at: DateTime,
}

#[derive(State)]
pub(crate) struct DateTimePage {
    clock: Option<TimeDate>,
    state: rikka_timedate::State,
    zones: Vec<Zone>,
    #[state]
    ticking: Ticking,
    choosing: Option<TextDocument>,
    servers: TextDocument,
    manual: Option<Manual>,
}

impl Page for DateTimePage {
    fn opened(&mut self, pending: &Pending, _: &Session) {
        let clock = self.clock.get_or_insert_with(TimeDate::start);
        clock.reread();
        self.state = clock.state();
        stream_keyed(
            FOLLOWING,
            clock.changes(),
            |page: &mut DateTimePage, state: rikka_timedate::State| page.follow(state),
        );
        self.servers = TextDocument::new(servers(pending).join(" "));
        self.choosing = None;
        self.manual = None;
        self.ticking.start();
        if self.zones.is_empty() {
            spawn_blocking(zones::installed, |page: &mut DateTimePage, zones| {
                page.zones = zones;
            });
        }
    }

    fn closed(&mut self) {
        cancel(FOLLOWING);
        self.ticking.stop();
    }
}

impl DateTimePage {
    pub(crate) fn new() -> Self {
        Self {
            clock: None,
            state: rikka_timedate::State::default(),
            zones: Vec::new(),
            ticking: Ticking::new(),
            choosing: None,
            servers: TextDocument::new(""),
            manual: None,
        }
    }

    pub(crate) fn state(&self) -> &rikka_timedate::State {
        &self.state
    }

    pub(crate) fn ticking(&self) -> &Ticking {
        &self.ticking
    }

    pub(crate) fn servers(&self) -> &TextDocument {
        &self.servers
    }

    pub(crate) fn choosing(&self) -> Option<&TextDocument> {
        self.choosing.as_ref()
    }

    pub(crate) fn manual(&self) -> Manual {
        self.manual.unwrap_or_else(|| Manual {
            at: Timestamp::now().to_zoned(TimeZone::system()).datetime(),
        })
    }

    pub(crate) fn zone(&self, name: &str) -> Option<Zone> {
        self.zones
            .iter()
            .find(|zone| zone.name == name)
            .cloned()
            .or_else(|| Zone::named(name))
    }

    pub(crate) fn offered(&self) -> Vec<&Zone> {
        let query = self
            .choosing
            .as_ref()
            .map(|typed| typed.text().trim().to_lowercase())
            .unwrap_or_default();
        self.zones
            .iter()
            .filter(|zone| zone.matches(&query))
            .collect()
    }

    pub(crate) fn open_chooser(&mut self) {
        self.choosing = Some(TextDocument::new(""));
    }

    pub(crate) fn close_chooser(&mut self) {
        self.choosing = None;
    }

    pub(crate) fn choose_zone(&mut self, pending: &mut Pending, index: usize) {
        let Some(name) = self.offered().get(index).map(|zone| zone.name.clone()) else {
            return;
        };
        self.choosing = None;
        pending.set(Module::System, TIME_ZONE, name);
    }

    pub(crate) fn adjust(&mut self, part: Part, value: f32) {
        let at = self.manual().at;
        let value = value.round() as i64;
        let (year, month, day, hour, minute) = (
            i64::from(at.year()),
            i64::from(at.month()),
            i64::from(at.day()),
            i64::from(at.hour()),
            i64::from(at.minute()),
        );
        let (year, month, day, hour, minute) = match part {
            Part::Year => (value, month, day, hour, minute),
            Part::Month => (year, value, day, hour, minute),
            Part::Day => (year, month, value, hour, minute),
            Part::Hour => (year, month, day, value, minute),
            Part::Minute => (year, month, day, hour, value),
        };
        let Some(at) = civil(year, month, day, hour, minute) else {
            return;
        };
        self.manual = Some(Manual { at });
    }

    pub(crate) fn set_clock(&mut self) {
        let (Some(clock), Some(manual)) = (&self.clock, self.manual) else {
            return;
        };
        let Ok(zoned) = manual.at.to_zoned(TimeZone::system()) else {
            return;
        };
        clock.set_time(SystemTime::from(zoned.timestamp()));
        self.manual = None;
    }

    pub(crate) fn commit_servers(&mut self, pending: &mut Pending) {
        let typed = self.servers.text();
        let named: Vec<String> = typed.split_whitespace().map(str::to_owned).collect();
        if !named.iter().all(|server| host_name::is_name(server)) || named == servers(pending) {
            return;
        }
        match named.is_empty() {
            true => pending.remove(Module::System, SERVERS),
            false => pending.set(Module::System, SERVERS, named),
        }
    }

    fn follow(&mut self, state: rikka_timedate::State) {
        if self.state.ntp && !state.ntp {
            self.manual = None;
        }
        self.state = state;
    }
}

pub(crate) fn servers(pending: &Pending) -> Vec<String> {
    pending
        .get::<Vec<String>>(Module::System, SERVERS)
        .unwrap_or_default()
}

pub(crate) fn servers_typed_well(typed: &str) -> bool {
    typed.split_whitespace().all(host_name::is_name)
}

pub(crate) fn synchronizes(pending: &Pending) -> bool {
    pending
        .get::<bool>(Module::System, SYNCHRONIZED)
        .unwrap_or(true)
}

pub(crate) fn locates(pending: &Pending) -> bool {
    pending
        .get::<bool>(Module::System, AUTOMATIC_ZONE)
        .unwrap_or(false)
}

pub(crate) fn keeps_local_rtc(pending: &Pending) -> bool {
    pending
        .get::<bool>(Module::System, LOCAL_RTC)
        .unwrap_or(false)
}

pub(crate) fn set_synchronized(pending: &mut Pending, on: bool) {
    match on {
        true => pending.remove(Module::System, SYNCHRONIZED),
        false => pending.set(Module::System, SYNCHRONIZED, false),
    }
}

pub(crate) fn set_local_rtc(pending: &mut Pending, on: bool) {
    match on {
        true => pending.set(Module::System, LOCAL_RTC, true),
        false => pending.remove(Module::System, LOCAL_RTC),
    }
}

pub(crate) fn set_locating(pending: &mut Pending, on: bool, current: Option<String>) {
    pending.change(Module::System, |document| {
        match on {
            true => {
                document.set(AUTOMATIC_ZONE, true)?;
                document.remove(TIME_ZONE)?;
            }
            false => {
                document.remove(AUTOMATIC_ZONE)?;
                if let Some(zone) = current {
                    document.set(TIME_ZONE, zone)?;
                }
            }
        }
        Ok(())
    });
}

fn civil(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> Option<DateTime> {
    let year = i16::try_from(year).ok()?;
    let month = i8::try_from(month.clamp(1, 12)).ok()?;
    let first = jiff::civil::Date::new(year, month, 1).ok()?;
    let day = i8::try_from(day.clamp(1, i64::from(first.days_in_month()))).ok()?;
    let hour = i8::try_from(hour.clamp(0, 23)).ok()?;
    let minute = i8::try_from(minute.clamp(0, 59)).ok()?;
    DateTime::new(year, month, day, hour, minute, 0, 0).ok()
}

impl std::fmt::Debug for DateTimePage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DateTimePage")
            .field("state", &self.state)
            .field("zones", &self.zones.len())
            .finish_non_exhaustive()
    }
}
