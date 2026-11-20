use std::time::Duration;

use rikka_app::WindowShape;
use rikka_app::prelude::*;

use crate::requests;
use crate::shell::Shell;
use crate::site_info::{self, SiteInfo};
use crate::widgets::presence;
use browser_model::TabId;

const HINT: Duration = Duration::from_secs(3);
const HINT_TIMER: &str = "rikka-browser-hint";
const POPUP_KEY: &str = "rikka-browser-popup";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Popup {
    SiteInfo(TabId),
    Downloads,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Fullscreen {
    #[default]
    Off,
    Window,
    Page {
        tab: TabId,
        window: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hint {
    LeavePage,
    LeaveWindow,
}

#[derive(State)]
pub(crate) struct Overlays {
    popup: Tracked<Option<Popup>>,
    fullscreen: Tracked<Fullscreen>,
    hint: Tracked<Option<Hint>>,
    #[state]
    pub(crate) site: SiteInfo,
}

impl Overlays {
    pub(crate) fn new() -> Overlays {
        Overlays {
            popup: Tracked::new(None),
            fullscreen: Tracked::new(Fullscreen::Off),
            hint: Tracked::new(None),
            site: SiteInfo::new(),
        }
    }

    pub(crate) fn popup(&self) -> Option<Popup> {
        *self.popup
    }

    pub(crate) fn fullscreen(&self) -> Fullscreen {
        *self.fullscreen
    }

    pub(crate) fn hint(&self) -> Option<Hint> {
        *self.hint
    }

    pub(crate) fn hides_chrome(&self) -> bool {
        *self.fullscreen != Fullscreen::Off
    }

    pub(crate) fn close_popup(&mut self) {
        if self.popup.is_some() {
            self.popup.set(None);
        }
    }

    fn show_hint(&mut self, hint: Hint) {
        self.hint.set(Some(hint));
        after_keyed(HINT_TIMER, HINT, |overlays: &mut Overlays| {
            overlays.hint.set(None)
        });
    }
}

impl std::fmt::Debug for Overlays {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Overlays")
            .field("popup", &*self.popup)
            .field("fullscreen", &*self.fullscreen)
            .finish_non_exhaustive()
    }
}

impl Shell {
    pub(crate) fn show_downloads(&mut self) {
        self.overlays.popup.set(Some(Popup::Downloads));
        self.load_downloads();
    }

    pub(crate) fn toggle_downloads(&mut self) {
        match self.overlays.popup() {
            Some(Popup::Downloads) => self.overlays.close_popup(),
            _ => self.show_downloads(),
        }
    }

    pub(crate) fn open_site_info(&mut self) {
        let active = self.active_tab();
        if let Some(tab) = active {
            self.open_site_info_of(tab);
        }
    }

    pub(crate) fn open_site_info_of(&mut self, tab: TabId) {
        if self.overlays.popup() == Some(Popup::SiteInfo(tab)) {
            self.overlays.close_popup();
            return;
        }
        self.overlays.popup.set(Some(Popup::SiteInfo(tab)));
        self.reset_site_info();
    }

    pub(crate) fn toggle_fullscreen(&mut self) {
        match self.overlays.fullscreen() {
            Fullscreen::Off => {
                self.overlays.fullscreen.set(Fullscreen::Window);
                self.overlays.close_popup();
                request_window(WindowRequest::Fullscreen(true));
                self.overlays.show_hint(Hint::LeaveWindow);
            }
            Fullscreen::Window => {
                self.overlays.fullscreen.set(Fullscreen::Off);
                request_window(WindowRequest::Fullscreen(false));
            }
            Fullscreen::Page { tab, .. } => {
                if let Some(view) = self.view_of(tab) {
                    view.exit_fullscreen();
                }
            }
        }
    }

    pub(crate) fn page_fullscreen(&mut self, tab: TabId, on: bool) {
        if !on {
            self.page_left_fullscreen(tab);
            return;
        }
        let active = self.active_tab();
        if active != Some(tab) {
            if let Some(view) = self.view_of(tab) {
                view.exit_fullscreen();
            }
            return;
        }
        let window = match self.overlays.fullscreen() {
            Fullscreen::Window => true,
            Fullscreen::Page { window, .. } => window,
            Fullscreen::Off => false,
        };
        self.overlays
            .fullscreen
            .set(Fullscreen::Page { tab, window });
        self.overlays.close_popup();
        if !window {
            request_window(WindowRequest::Fullscreen(true));
        }
        self.overlays.show_hint(Hint::LeavePage);
    }

    pub(crate) fn page_left_fullscreen(&mut self, tab: TabId) {
        let Fullscreen::Page {
            tab: fullscreen_tab,
            window,
        } = self.overlays.fullscreen()
        else {
            return;
        };
        if fullscreen_tab != tab {
            return;
        }
        cancel(HINT_TIMER);
        self.overlays.hint.set(None);
        match window {
            true => self.overlays.fullscreen.set(Fullscreen::Window),
            false => {
                self.overlays.fullscreen.set(Fullscreen::Off);
                request_window(WindowRequest::Fullscreen(false));
            }
        }
    }

    pub(crate) fn follow_fullscreen(&mut self, active: Option<TabId>) {
        if let Fullscreen::Page { tab, .. } = self.overlays.fullscreen()
            && active != Some(tab)
        {
            if let Some(view) = self.view_of(tab) {
                view.exit_fullscreen();
            }
            self.page_left_fullscreen(tab);
        }
    }

    pub(crate) fn shaped(&mut self, shape: &WindowShape) {
        if shape.fullscreen || self.overlays.fullscreen() == Fullscreen::Off {
            return;
        }
        if let Fullscreen::Page { tab, .. } = self.overlays.fullscreen()
            && let Some(view) = self.view_of(tab)
        {
            view.exit_fullscreen();
        }
        self.overlays.fullscreen.set(Fullscreen::Off);
        self.overlays.hint.set(None);
    }
}

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let active = model.window(shell.id()).and_then(|layout| layout.active());
    drop(model);
    let peeking = shell.peek.shown();
    let badge_of = |tab: TabId| match (Some(tab) == peeking, Some(tab) == active) {
        (true, _) => Some(site_info::PEEK_BADGE),
        (false, true) => Some(site_info::BADGE),
        (false, false) => None,
    };
    let popup = shell.overlays.popup().and_then(|popup| {
        let (panel, anchor) = match popup {
            Popup::Downloads => (
                crate::downloads::panel(shell),
                Anchor::new(crate::downloads::BUTTON, AnchorEdge::BelowEnd).offset(0.0, 6.0),
            ),
            Popup::SiteInfo(tab) => {
                let badge = badge_of(tab)?;
                (
                    site_info::view(shell, tab),
                    Anchor::below(badge).offset(-4.0, 8.0),
                )
            }
        };
        Some(
            popover(panel)
                .on_dismiss(|shell: &mut Shell| shell.overlays.close_popup())
                .build()
                .keyed((POPUP_KEY, popup))
                .anchored(anchor),
        )
    });
    let prompt = match popup.is_some() {
        true => None,
        false => {
            let page_services = shell.browser().page_services();
            let requests = page_services.requests();
            [peeking, active].into_iter().flatten().find_map(|tab| {
                let prompt = requests::prompt(requests.get(tab)?, tab)?;
                Some(prompt.anchored(Anchor::below(badge_of(tab)?).offset(-4.0, 8.0)))
            })
        }
    };
    presence([popup, prompt].into_iter().flatten())
}

pub(crate) fn hint(shell: &Shell) -> Option<Element> {
    let label = match shell.overlays.hint()? {
        Hint::LeavePage => tr!("browser.fullscreen.leave-page"),
        Hint::LeaveWindow => tr!("browser.fullscreen.leave-window"),
    };
    Some(
        container(text(label).label().color(Color::WHITE))
            .padding(Inset::xy(Space::WIDE, Space::NORMAL))
            .background(Color::BLACK.with_alpha(0.72))
            .radius(999.0)
            .transition(fade())
            .any(),
    )
}

pub(crate) fn sidebar(shell: &Shell) -> Element {
    match shell.overlays.hides_chrome() {
        true => Element::empty(),
        false => crate::sidebar::side(shell),
    }
}
