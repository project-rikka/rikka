use rikka_app::prelude::*;
use rikka_settings::session::windows::{
    ALLOW_TEARING, DOMAIN, FLOATING, FOCUS_OUTLINE, GAP_INNER, GAP_MAX, GAP_OUTER, LAYOUT,
    RESIZE_ON_BORDER, SCROLLING, SNAP, SNAP_ASSIST, SNAP_BAR, SNAP_TOGETHER, TILING,
};
use rikka_widgets::column;

use crate::controls::{Choices, Count, choice, flag};
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::session::Session;

const LAYOUTS: Choices = &[
    (FLOATING, "settings.windows.floating"),
    (TILING, "settings.windows.tiling"),
];

pub(crate) struct Windows {
    domain: Domain,
}

impl Page for Windows {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
    }
}

domain_page!(Windows);

impl Windows {
    pub(crate) fn new() -> Self {
        Self {
            domain: Domain::new(DOMAIN),
        }
    }

    fn layout(&self) -> &'static str {
        self.domain.one_of(LAYOUT, LAYOUTS)
    }
}

pub(crate) fn view(windows: &Windows) -> impl View {
    column! {
        domain_notice(
            &windows.domain,
            "settings.windows.unavailable",
            "settings.windows.refused",
        );
        controls(windows);
    }
    .gap(Space::ROOMY)
}

fn controls(windows: &Windows) -> impl View {
    column! {
        choice(windows, tr!("settings.windows.layout"), LAYOUT, LAYOUTS);
        flag(windows, tr!("settings.windows.resize-on-border"), RESIZE_ON_BORDER);
        if windows.layout() == TILING {
            flag(windows, tr!("settings.windows.scrolling"), SCROLLING);
            flag(windows, tr!("settings.windows.focus-outline"), FOCUS_OUTLINE);
            gap(windows, tr!("settings.windows.gap-inner"), GAP_INNER);
            gap(windows, tr!("settings.windows.gap-outer"), GAP_OUTER);
        } else {
            flag(windows, tr!("settings.windows.snap"), SNAP);
            if windows.domain.get(SNAP).unwrap_or(false) {
                flag(windows, tr!("settings.windows.snap-bar"), SNAP_BAR);
                flag(windows, tr!("settings.windows.snap-assist"), SNAP_ASSIST);
                flag(windows, tr!("settings.windows.snap-resize-together"), SNAP_TOGETHER);
            }
        }
        flag(windows, tr!("settings.windows.allow-tearing"), ALLOW_TEARING);
    }
    .gap(Space::SNUG)
}

fn gap(windows: &Windows, label: Label, key: &'static str) -> Field {
    Count {
        key,
        absent: 0,
        range: 0.0..=GAP_MAX as f32,
    }
    .field(windows, label, |pixels| format!("{pixels} px"))
}

impl std::fmt::Debug for Windows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Windows")
            .field("layout", &self.layout())
            .finish_non_exhaustive()
    }
}
