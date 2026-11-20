mod catch;
mod inert;
mod pinch;
mod space_swipe;

pub(crate) use catch::catch_gestures;
pub(crate) use inert::inert;
pub(crate) use space_swipe::space_swipe;

use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    Back,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SwipePhase {
    Moved,
    Committed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Swipe {
    pub(crate) direction: Direction,
    pub(crate) progress: f32,
    pub(crate) phase: SwipePhase,
}

use browser_pages::Cause;

use crate::shell::Shell;
use browser_model::TabId;

use pinch::Pinching;

const MARK: f32 = 44.0;
const FULL_SWIPE: f32 = 72.0;
const GLYPH: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Swiping {
    tab: TabId,
    direction: Direction,
    progress: f32,
}

#[derive(State)]
pub(crate) struct Gestures {
    swiping: Tracked<Option<Swiping>>,
    #[untracked]
    pinching: Option<Pinching>,
}

impl Gestures {
    pub(crate) fn new() -> Gestures {
        Gestures {
            swiping: Tracked::new(None),
            pinching: None,
        }
    }

    pub(crate) fn swipe(&mut self, tab: TabId, swipe: Swipe) -> Option<Direction> {
        let shown = match swipe.phase {
            SwipePhase::Moved => Some(Swiping {
                tab,
                direction: swipe.direction,
                progress: swipe.progress.clamp(0.0, 1.0),
            }),
            SwipePhase::Committed | SwipePhase::Cancelled => None,
        };
        self.swiping.set(shown);
        (swipe.phase == SwipePhase::Committed).then_some(swipe.direction)
    }
}

impl std::fmt::Debug for Gestures {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Gestures")
            .field("swiping", &*self.swiping)
            .field("pinching", &self.pinching)
            .finish()
    }
}

impl Shell {
    pub(crate) fn swiped(&mut self, tab: TabId, swipe: Swipe) {
        let Some(direction) = self.gestures.swipe(tab, swipe) else {
            return;
        };
        let placed = self
            .model()
            .window(self.id())
            .is_some_and(|layout| layout.holds(tab));
        if !placed {
            let view = {
                let mut model = self.browser().edit();
                let Some(tab_entry) = model.tab_mut(tab) else {
                    return;
                };
                tab_entry.visit_tracker.set_cause(Cause::BackForward);
                tab_entry.page().view().cloned()
            };
            match (view, direction) {
                (Some(view), Direction::Back) => view.go_back(),
                (Some(view), Direction::Forward) => view.go_forward(),
                (None, _) => {}
            }
            return;
        }
        if self.active_tab() != Some(tab) {
            self.select(tab);
        }
        match direction {
            Direction::Back => self.back(),
            Direction::Forward => self.forward(),
        }
    }

    pub(crate) fn pinched(&mut self, tab: TabId, phase: GesturePhase, scale: f32) {
        let Some(view) = self.view_of(tab) else {
            return;
        };
        match phase {
            GesturePhase::Began => {
                self.gestures.pinching = Some(Pinching::begin(tab, view.page().zoom));
            }
            GesturePhase::Moved => {
                let followed = self
                    .gestures
                    .pinching
                    .as_mut()
                    .filter(|pinching| pinching.tab == tab)
                    .and_then(|pinching| pinching.follow(scale));
                if let Some(zoom) = followed {
                    view.set_zoom(zoom);
                }
            }
            GesturePhase::Ended => {
                let Some(pinching) = self
                    .gestures
                    .pinching
                    .take()
                    .filter(|candidate| candidate.tab == tab)
                else {
                    return;
                };
                let zoom = pinching.settled();
                view.set_zoom(zoom);
                let default = self.browser().preferences().default_zoom();
                self.browser().edit().remember_zoom(tab, zoom, default);
            }
            GesturePhase::Cancelled => {
                if let Some(pinching) = self
                    .gestures
                    .pinching
                    .take()
                    .filter(|candidate| candidate.tab == tab)
                {
                    view.set_zoom(pinching.from);
                }
            }
        }
    }
}

pub(crate) fn indicator(shell: &Shell, tab: TabId) -> Option<Element> {
    let swiping = (*shell.gestures.swiping).filter(|swiping| swiping.tab == tab)?;
    let reached = swiping.progress >= 1.0;
    let (symbol, justify, travel) = match swiping.direction {
        Direction::Back => (MaterialSymbol::ARROW_BACK, Justify::Start, 1.0),
        Direction::Forward => (MaterialSymbol::ARROW_FORWARD, Justify::End, -1.0),
    };
    let (fill, ink) = match reached {
        true => (palette().accent, palette().on_accent),
        false => (palette().surface, palette().text),
    };
    let mark = container(icon(symbol).extent(Length::px(GLYPH)).color(ink))
        .center()
        .background(fill)
        .border(1.0, palette().outline.with_alpha(0.4))
        .radius(MARK / 2.0)
        .size(Length::px(MARK), Length::px(MARK))
        .build()
        .offset(travel * (swiping.progress * FULL_SWIPE - MARK), 0.0)
        .opacity(0.4 + 0.6 * swiping.progress);
    Some(
        inert(
            row_of([mark])
                .justify(justify)
                .align_items(Align::Center)
                .fill(),
        )
        .transition(fade()),
    )
}
