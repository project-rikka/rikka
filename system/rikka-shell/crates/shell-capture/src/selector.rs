use std::sync::Arc;

use rikka_seam::Request;
use rikka_session::following;
use rikka_types::Rect;
use rikka_ui::{Element, ImageSource, State, View, is_exiting, stream};
use shell_kit::Placement;

use crate::aim::Mode;
use crate::overlay::{Marquee, marquee};
use crate::scan::{self, Act, Acting};
use crate::service::{ACT, Area, CANCEL, CAPABILITY, CHANGED, CHOSE, Chosen, POINT};
use crate::shared::Shared;

#[derive(State)]
pub(super) struct Selector {
    shared: Shared,
    place: Placement,
    picture: Option<ImageSource>,
}

impl Selector {
    #[must_use]
    pub(crate) fn new(shared: Shared, place: Placement) -> Selector {
        let picture = shared
            .frozen
            .borrow()
            .as_deref()
            .and_then(|frozen| frozen.panel(&place.name))
            .map(|panel| ImageSource::Rgba {
                width: panel.width,
                height: panel.height,
                pixels: Arc::from(&panel.pixels[..]),
            });
        Selector {
            shared,
            place,
            picture,
        }
    }

    pub(crate) fn start(&mut self) {
        let announced = following::changes(&self.shared.seam, CAPABILITY, CHANGED);
        stream(announced, |_: &mut Selector, ()| {});
    }

    pub(crate) fn picked(&mut self, area: Rect, window: Option<String>) {
        self.shared
            .seam
            .notify(Request::new(CAPABILITY, CHOSE).encode(&Chosen {
                area: Area::from(area),
                window: window.unwrap_or_default(),
            }));
    }

    pub(crate) fn cancelled(&mut self) {
        self.shared.seam.notify(Request::new(CAPABILITY, CANCEL));
    }

    pub(crate) fn point(&mut self, code: Option<usize>) {
        self.shared
            .seam
            .notify(Request::new(CAPABILITY, POINT).encode(&code));
    }

    pub(crate) fn act(&mut self, code: usize, act: Act) {
        self.shared
            .seam
            .notify(Request::new(CAPABILITY, ACT).encode(&Acting { code, act }));
    }

    pub(crate) fn primary(&mut self) {
        let chosen = self
            .shared
            .codes
            .read()
            .chosen()
            .map(|(index, code)| (index, Act::primary(&code.meaning)));
        if let Some((index, act)) = chosen {
            self.act(index, act);
        }
    }

    pub(crate) fn copy(&mut self) {
        let chosen = self.shared.codes.read().chosen;
        if let Some(index) = chosen {
            self.act(index, Act::Copy);
        }
    }

    pub(crate) fn cycle(&mut self, forward: bool) {
        let (count, chosen) = {
            let decoded = self.shared.codes.read();
            (decoded.codes.len(), decoded.chosen)
        };
        if count == 0 {
            return;
        }
        let next = match (chosen, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
        };
        self.point(Some(next));
    }

    pub(crate) fn dismiss(&mut self) {
        let shown = {
            let decoded = self.shared.codes.read();
            decoded.chosen.is_some() || decoded.missed.is_some()
        };
        match shown {
            true => self.point(None),
            false => self.cancelled(),
        }
    }

    fn card(&self) -> Option<Element> {
        if self.shared.aimed.mode() != Mode::Scan {
            return None;
        }
        let output = Rect::from_origin_size(self.place.position, self.place.size);
        let decoded = self.shared.codes.read();
        if let Some((index, code)) = decoded.chosen() {
            if !output.contains(code.area.center()) {
                return None;
            }
            let area = self.place.to_surface(code.area);
            return Some(scan::beside(area, scan::card(index, code, decoded.trouble)).build());
        }
        let missed = decoded
            .missed
            .filter(|area| output.contains(area.center()))?;
        Some(scan::beside(self.place.to_surface(missed), scan::missed()).build())
    }
}

pub(crate) fn view(selector: &Selector) -> Marquee {
    marquee(
        selector.place.clone(),
        selector.shared.clone(),
        selector.picture.clone(),
    )
    .overlay(selector.card())
    .leaving(is_exiting())
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selector")
            .field("output", &self.place.name)
            .finish_non_exhaustive()
    }
}
