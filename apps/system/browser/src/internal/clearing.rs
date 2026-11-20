use std::time::Duration;

use browser_store::{DataKinds, TimeRange, Timestamp};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::shell::Shell;

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
const CHOICE: f32 = 220.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Span {
    #[default]
    Hour,
    Day,
    Week,
    FourWeeks,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Kinds {
    pub(crate) history: bool,
    pub(crate) downloads: bool,
    pub(crate) site_data: bool,
    pub(crate) cache: bool,
    pub(crate) site_settings: bool,
    pub(crate) tabs: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Phase {
    #[default]
    Ready,
    Clearing,
    Cleared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    History,
    Downloads,
    SiteData,
    Cache,
    SiteSettings,
    Tabs,
}

#[derive(State, Debug)]
pub(crate) struct Clearing {
    span: Tracked<Span>,
    kinds: Tracked<Kinds>,
    phase: Tracked<Phase>,
}

impl Span {
    pub(crate) const ALL: [Span; 5] = [
        Span::Hour,
        Span::Day,
        Span::Week,
        Span::FourWeeks,
        Span::All,
    ];

    pub(crate) fn length(self) -> Option<Duration> {
        let seconds = match self {
            Span::Hour => HOUR,
            Span::Day => DAY,
            Span::Week => 7 * DAY,
            Span::FourWeeks => 28 * DAY,
            Span::All => return None,
        };
        Some(Duration::from_secs(seconds))
    }

    pub(crate) fn range(self, now: Timestamp) -> TimeRange {
        match self.length() {
            Some(length) => TimeRange::since(now.saturating_sub(length)),
            None => TimeRange::all(),
        }
    }

    pub(crate) fn name(self) -> Label {
        match self {
            Span::Hour => tr!("browser.clear.hour"),
            Span::Day => tr!("browser.clear.day"),
            Span::Week => tr!("browser.clear.week"),
            Span::FourWeeks => tr!("browser.clear.four-weeks"),
            Span::All => tr!("browser.clear.all"),
        }
    }
}

impl Default for Kinds {
    fn default() -> Kinds {
        Kinds {
            history: true,
            downloads: false,
            site_data: true,
            cache: true,
            site_settings: false,
            tabs: false,
        }
    }
}

impl Kinds {
    pub(crate) fn to_data_kinds(self) -> DataKinds {
        DataKinds {
            history: self.history,
            downloads: self.downloads,
            closed_tabs: self.tabs,
            archive: self.tabs,
            site_settings: self.site_settings,
        }
    }

    pub(crate) fn web(self) -> rikka_web::ClearFlags {
        let mut kinds = rikka_web::ClearFlags::NONE;
        if self.site_data {
            kinds |= rikka_web::ClearFlags::COOKIES
                | rikka_web::ClearFlags::DOM_STORAGES
                | rikka_web::ClearFlags::AUTH_SESSIONS;
        }
        if self.cache {
            kinds |= rikka_web::ClearFlags::ALL_CACHES;
        }
        if self.site_settings {
            kinds |= rikka_web::ClearFlags::PERMISSIONS;
        }
        kinds
    }

    fn any(self) -> bool {
        self.history
            || self.downloads
            || self.site_data
            || self.cache
            || self.site_settings
            || self.tabs
    }

    fn get(self, kind: Kind) -> bool {
        match kind {
            Kind::History => self.history,
            Kind::Downloads => self.downloads,
            Kind::SiteData => self.site_data,
            Kind::Cache => self.cache,
            Kind::SiteSettings => self.site_settings,
            Kind::Tabs => self.tabs,
        }
    }

    fn with(mut self, kind: Kind, on: bool) -> Kinds {
        match kind {
            Kind::History => self.history = on,
            Kind::Downloads => self.downloads = on,
            Kind::SiteData => self.site_data = on,
            Kind::Cache => self.cache = on,
            Kind::SiteSettings => self.site_settings = on,
            Kind::Tabs => self.tabs = on,
        }
        self
    }
}

impl Kind {
    const ALL: [Kind; 6] = [
        Kind::History,
        Kind::Downloads,
        Kind::SiteData,
        Kind::Cache,
        Kind::SiteSettings,
        Kind::Tabs,
    ];

    fn name(self) -> Label {
        match self {
            Kind::History => tr!("browser.clear.history"),
            Kind::Downloads => tr!("browser.clear.downloads"),
            Kind::SiteData => tr!("browser.clear.site-data"),
            Kind::Cache => tr!("browser.clear.cache"),
            Kind::SiteSettings => tr!("browser.clear.site-settings"),
            Kind::Tabs => tr!("browser.clear.tabs"),
        }
    }
}

impl Clearing {
    pub(crate) fn new() -> Clearing {
        Clearing {
            span: Tracked::new(Span::default()),
            kinds: Tracked::new(Kinds::default()),
            phase: Tracked::new(Phase::Ready),
        }
    }

    pub(crate) fn chosen(&self) -> (Span, Kinds) {
        (*self.span, *self.kinds)
    }

    pub(crate) fn reset(&mut self) {
        self.phase.set(Phase::Ready);
    }

    pub(crate) fn begin(&mut self) {
        self.phase.set(Phase::Clearing);
    }

    pub(crate) fn finish(&mut self) {
        self.phase.set(Phase::Cleared);
    }

    fn choose_span(&mut self, at: usize) {
        if let Some(span) = Span::ALL.get(at) {
            self.span.set(*span);
            self.phase.set(Phase::Ready);
        }
    }

    fn choose(&mut self, kind: Kind, on: bool) {
        let kinds = self.kinds.with(kind, on);
        self.kinds.set(kinds);
        self.phase.set(Phase::Ready);
    }
}

pub(crate) fn view(shell: &Shell) -> Element {
    if shell.private() {
        return status_page(MaterialSymbol::DELETE_SWEEP, tr!("browser.clear.private")).any();
    }
    let clearing = &shell.pages().clearing;
    let (span, kinds) = clearing.chosen();
    let phase = *clearing.phase;
    let choices = Kind::ALL.map(|kind| {
        checkbox(kinds.get(kind))
            .label(text(kind.name()))
            .on_toggle(move |clearing: &mut Clearing, on: bool| clearing.choose(kind, on))
            .any()
    });
    let action = button(text(tr!("browser.clear.action")))
        .primary()
        .loading(phase == Phase::Clearing)
        .enabled(kinds.any() && phase != Phase::Clearing)
        .on_press(|shell: &mut Shell| shell.clear_browsing_data());
    let done = (phase == Phase::Cleared).then(|| {
        row! {
            icon(MaterialSymbol::CHECK_CIRCLE).extent(Length::px(16.0)).color(palette().success);
            text(tr!("browser.clear.done")).label().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
    });
    page([section(
        tr!("browser.clear.title"),
        column! {
            field(
                tr!("browser.clear.range"),
                combo(Span::ALL.iter().position(|candidate| *candidate == span))
                    .options(Span::ALL.map(Span::name))
                    .on_select(|clearing: &mut Clearing, at: usize| clearing.choose_span(at))
                    .width(CHOICE),
            );
            column_of(choices).gap(Space::SNUG).fill_width();
            row! {
                action;
                done;
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center);
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}
