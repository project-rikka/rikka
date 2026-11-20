use std::time::Duration;

use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_seam::SeamClient;
use rikka_settings::session::displays::{AdaptiveSync, SDR_BRIGHTNESS_RANGE};
use rikka_settings::{Error, Result};
use rikka_types::Choice;
use settings_displays::{Hdr, Monitor, Orientation, Rotation, Store};

use crate::amount::dimmed_amount;

use crate::displays::canvas::{self, Canvas, Tile};
use crate::displays::{adaptive_sync, format, rotation};
use crate::notice::notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::picker;
use crate::session::Session;

const POLL: Duration = Duration::from_secs(5);

const POLLING: &str = "settings.displays.polling";
const READING: &str = "settings.displays.reading";
const SCALES: [u32; 8] = [100, 125, 150, 175, 200, 225, 250, 300];

enum Unavailable {
    NotLoaded,
    Failed(Error),
}

pub(crate) struct Displays {
    seam: Option<SeamClient>,
    store: std::result::Result<Store, Unavailable>,
    session: Vec<Monitor>,
    draft: Vec<Monitor>,
    selected: Option<String>,
    error: Option<Error>,
}

impl Page for Displays {
    fn opened(&mut self, _: &Pending, session: &Session) {
        match session.seam(Store::DOMAIN) {
            Ok(seam) => self.seam = Some(seam),
            Err(error) => {
                self.store = Err(Unavailable::Failed(error));
                return;
            }
        }
        self.reload();
        every_keyed(POLLING, POLL, |displays: &mut Displays| {
            displays.read();
            skip_view();
        });
    }

    fn closed(&mut self) {
        cancel(POLLING);
        cancel(READING);
    }
}

impl Displays {
    pub(crate) fn new() -> Self {
        Self {
            seam: None,
            store: Err(Unavailable::NotLoaded),
            session: Vec::new(),
            draft: Vec::new(),
            selected: None,
            error: None,
        }
    }

    fn pending(&self) -> bool {
        self.draft != self.session
    }

    fn monitor(&self) -> Option<&Monitor> {
        self.draft.get(self.selected_index()?)
    }

    fn selected_index(&self) -> Option<usize> {
        let slug = self.selected.as_deref()?;
        self.draft.iter().position(|monitor| monitor.slug == slug)
    }

    fn edit(&mut self, change: impl FnOnce(&mut Monitor)) {
        let Some(index) = self.selected_index() else {
            return;
        };
        change(&mut self.draft[index]);
    }

    fn apply(&mut self) {
        let Ok(store) = &mut self.store else {
            return;
        };
        let mut error = None;
        for staged in &self.draft {
            let Some(current) = self
                .session
                .iter()
                .find(|monitor| monitor.slug == staged.slug)
            else {
                continue;
            };
            if let Err(reason) = store.write(current, staged)
                && error.is_none()
            {
                error = Some(reason);
            }
        }
        self.error = error;
        self.reload();
        self.draft = self.session.clone();
    }

    fn reload(&mut self) {
        if let Some(seam) = self.seam.clone() {
            self.adopted(Store::adopt(seam));
        }
    }

    fn read(&self) {
        let Some(seam) = self.seam.clone() else {
            return;
        };
        spawn_blocking_keyed(
            READING,
            move || Store::adopt(seam),
            |displays: &mut Displays, store: Result<Store>| {
                if !displays.adopted(store) {
                    skip_view();
                }
            },
        );
    }

    fn adopted(&mut self, store: Result<Store>) -> bool {
        let session = store.as_ref().map(Store::monitors).unwrap_or_default();
        let unchanged = self.store.is_ok() && store.is_ok() && session == self.session;
        self.store = store.map_err(Unavailable::Failed);
        self.session = session;
        let rebuilt = self.rebuild_finished();
        !unchanged || rebuilt
    }

    fn rebuild_finished(&mut self) -> bool {
        let same = self.draft.len() == self.session.len()
            && self
                .draft
                .iter()
                .zip(&self.session)
                .all(|(draft, session)| draft.slug == session.slug);
        if !same {
            self.draft = self.session.clone();
        }
        let mut reselected = false;
        if self.monitor().is_none() {
            let first = self.draft.first().map(|monitor| monitor.slug.clone());
            reselected = first != self.selected;
            self.selected = first;
        }
        !same || reselected
    }

    fn brighten(&mut self, nits: f64) {
        let (Ok(store), Some(slug)) = (&mut self.store, self.selected.clone()) else {
            return;
        };
        if let Err(reason) = store.write_sdr_brightness(&slug, nits) {
            self.error = Some(reason);
            return;
        }
        for monitor in self.session.iter_mut().chain(self.draft.iter_mut()) {
            if monitor.slug == slug
                && let Some(hdr) = &mut monitor.hdr
            {
                hdr.sdr_brightness = nits;
            }
        }
    }

    fn select(&mut self, index: usize) {
        self.selected = self.draft.get(index).map(|monitor| monitor.slug.clone());
    }

    fn move_to(&mut self, index: usize, position: (i32, i32)) {
        if let Some(monitor) = self.draft.get_mut(index) {
            monitor.position = position;
        }
    }

    fn discard(&mut self) {
        self.draft = self.session.clone();
        self.error = None;
    }

    fn enabled_count(&self) -> usize {
        self.draft.iter().filter(|monitor| monitor.enabled).count()
    }
}

pub(crate) fn view(displays: &Displays) -> impl View {
    column! {
        header(displays);
        notice_of(displays);
        if !displays.draft.is_empty() {
            arrangement(displays);
            if let Some(at) = displays.selected_index()
                && let Some(monitor) = displays.draft.get(at)
            {
                page_stack(at, card(details(displays, monitor)))
                    .named(&monitor.slug)
                    .fill_width();
            }
        }
    }
    .gap(Space::ROOMY)
}

fn header(displays: &Displays) -> impl View {
    let pending = displays.pending();
    row! {
        spacer();
        button(tr!("settings.displays.discard"))
            .ghost()
            .enabled(pending)
            .on_press(|displays: &mut Displays| displays.discard());
        button(tr!("settings.displays.apply"))
            .primary()
            .enabled(pending)
            .on_press(|displays: &mut Displays| displays.apply());
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
}

fn arrangement(displays: &Displays) -> impl View {
    let selected = displays.selected_index();
    let tiles = displays
        .draft
        .iter()
        .enumerate()
        .map(|(index, monitor)| Tile {
            bounds: monitor.bounds(),
            number: index + 1,
            caption: format::size(monitor.mode.width, monitor.mode.height),
            selected: selected == Some(index),
            enabled: monitor.enabled,
        })
        .collect();
    Element::new(Canvas::new(
        tiles,
        |displays: &mut Displays, index: usize| displays.select(index),
        |displays: &mut Displays, index: usize, position: (i32, i32)| {
            displays.move_to(index, position);
        },
    ))
    .fill_width()
    .height(canvas::HEIGHT)
}

fn details(displays: &Displays, monitor: &Monitor) -> impl View {
    let resolutions = monitor.resolutions();
    let rates = monitor.refresh_rates();
    let scales = scale_ladder(monitor.scale);

    column! {
        column! {
            text(monitor.name.as_str()).body().semibold();
            text(tr!(
                "settings.displays.summary",
                size = &format::size(monitor.mode.width, monitor.mode.height),
                refresh = &format::refresh(monitor.mode),
            ))
            .label()
            .muted();
        }
        .gap(Space::TIGHT);
        field(
            tr!("settings.displays.enabled"),
            switch(monitor.enabled)
                .on_toggle(|displays: &mut Displays, enabled: bool| {
                    displays.edit(|monitor| monitor.enabled = enabled);
                })
                .enabled(!(displays.enabled_count() <= 1 && monitor.enabled)),
        );
        field(
            tr!("settings.displays.resolution"),
            picker(
                resolutions
                    .iter()
                    .position(|size| *size == (monitor.mode.width, monitor.mode.height)),
                resolutions
                    .iter()
                    .map(|(width, height)| Label::from(format::size(*width, *height))),
                |displays: &mut Displays, index: usize| {
                    displays.edit(|monitor| {
                        let Some(size) = monitor.resolutions().get(index).copied() else {
                            return;
                        };
                        if let Some(mode) = monitor
                            .modes
                            .iter()
                            .find(|mode| (mode.width, mode.height) == size)
                        {
                            monitor.mode = *mode;
                        }
                    });
                },
            ),
        );
        field(
            tr!("settings.displays.refresh"),
            picker(
                rates.iter().position(|mode| *mode == monitor.mode),
                rates.iter().map(|mode| Label::from(format::refresh(*mode))),
                |displays: &mut Displays, index: usize| {
                    displays.edit(|monitor| {
                        if let Some(mode) = monitor.refresh_rates().get(index) {
                            monitor.mode = *mode;
                        }
                    });
                },
            ),
        );
        field(
            tr!("settings.displays.scale"),
            picker(
                scales.iter().position(|scale| *scale == monitor.scale),
                scales
                    .iter()
                    .map(|scale| Label::from(format!("{}%", (scale * 100.0).round()))),
                |displays: &mut Displays, index: usize| {
                    displays.edit(|monitor| {
                        if let Some(scale) = scale_ladder(monitor.scale).get(index) {
                            monitor.scale = *scale;
                        }
                    });
                },
            ),
        );
        field(
            tr!("settings.displays.orientation"),
            picker(
                Some(monitor.orientation.rotation.index()),
                Rotation::ALL.map(rotation::label),
                |displays: &mut Displays, index: usize| {
                    displays.edit(|monitor| {
                        if let Some(rotation) = Rotation::ALL.get(index) {
                            monitor.orientation = Orientation {
                                rotation: *rotation,
                                flipped: monitor.orientation.flipped,
                            };
                        }
                    });
                },
            ),
        );
        field(
            tr!("settings.displays.flipped"),
            switch(monitor.orientation.flipped).on_toggle(
                |displays: &mut Displays, flipped: bool| {
                    displays.edit(|monitor| monitor.orientation.flipped = flipped);
                },
            ),
        );
        field(
            tr!("settings.displays.adaptive-sync"),
            picker(
                AdaptiveSync::ALL
                    .iter()
                    .position(|choice| *choice == monitor.adaptive_sync),
                AdaptiveSync::ALL.iter().copied().map(adaptive_sync::label),
                |displays: &mut Displays, index: usize| {
                    if let Some(choice) = AdaptiveSync::ALL.get(index).copied() {
                        displays.edit(|monitor| monitor.adaptive_sync = choice);
                    }
                },
            ),
        );
        if let Some(hdr) = monitor.hdr {
            high_dynamic_range(hdr);
        }
    }
    .gap(Space::SNUG)
}

fn high_dynamic_range(hdr: Hdr) -> impl View {
    let (low, high) = (
        *SDR_BRIGHTNESS_RANGE.start() as f32,
        *SDR_BRIGHTNESS_RANGE.end() as f32,
    );
    column! {
        field(
            tr!("settings.displays.hdr"),
            switch(hdr.enabled).on_toggle(|displays: &mut Displays, on: bool| {
                displays.edit(|monitor| {
                    if let Some(hdr) = &mut monitor.hdr {
                        hdr.enabled = on;
                    }
                });
            }),
        );
        field(
            tr!("settings.displays.sdr-brightness"),
            dimmed_amount(
                hdr.sdr_brightness as f32,
                low..=high,
                1.0,
                tr!(
                    "settings.displays.nits",
                    nits = &format!("{}", hdr.sdr_brightness.round())
                )
                .to_string(),
                |displays: &mut Displays, nits: f32| displays.brighten(f64::from(nits)),
                !hdr.enabled,
            ),
        );
    }
    .gap(Space::SNUG)
}

fn notice_of(displays: &Displays) -> Option<impl View> {
    let palette = palette();
    let (symbol, ink, message) = match (&displays.store, &displays.error) {
        (Err(Unavailable::NotLoaded), _) => (
            MaterialSymbol::CLOUD_OFF,
            palette.warning,
            tr!("settings.displays.unread"),
        ),
        (Err(Unavailable::Failed(reason)), _) => (
            MaterialSymbol::CLOUD_OFF,
            palette.warning,
            tr!(
                "settings.displays.unavailable",
                reason = &reason.to_string()
            ),
        ),
        (Ok(_), Some(reason)) => (
            MaterialSymbol::ERROR,
            palette.danger,
            tr!("settings.displays.refused", reason = &reason.to_string()),
        ),
        (Ok(_), None) if displays.session.is_empty() => (
            MaterialSymbol::CLOUD_OFF,
            palette.warning,
            tr!("settings.displays.none"),
        ),
        (Ok(_), None) if displays.pending() => (
            MaterialSymbol::EDIT,
            palette.accent,
            tr!("settings.displays.pending"),
        ),
        (Ok(_), None) => return None,
    };
    Some(notice(symbol, ink, message))
}

fn scale_ladder(current: f64) -> Vec<f64> {
    let mut ladder: Vec<f64> = SCALES
        .iter()
        .map(|percent| f64::from(*percent) / 100.0)
        .collect();
    if !ladder.contains(&current) {
        ladder.push(current);
        ladder.sort_by(|a, b| a.partial_cmp(b).expect("no scale is not a number"));
    }
    ladder
}

impl std::fmt::Debug for Displays {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Displays")
            .field("displays", &self.session.len())
            .field("selected", &self.selected)
            .field("pending", &self.pending())
            .finish_non_exhaustive()
    }
}
