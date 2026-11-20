mod cards;
mod frame;
mod input;
mod paint;
mod strip;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub(crate) use cards::Cards;
use input::{Drag, Dropped, Press, Scrolling, Spot};
use paint::DeckLayout;
use rikka_types::{Point, Rect};
use rikka_ui::{
    Behavior, Element, Event, EventCtx, ImageSource, Layout, Length, PaintCtx, View, Widget,
};
use shell_kit::DesktopFrost;
use shell_kit::taskbar::Shape;

use crate::swipe::Swipes;

pub(crate) struct Board {
    pub(crate) spaces: Vec<Space>,
    pub(crate) showing: usize,
    pub(crate) selected: Option<Arc<str>>,
    pub(crate) keyed: bool,
    pub(crate) output: Arc<str>,
    pub(crate) backdrop: Backdrop,
    pub(super) swipes: Swipes,
    pub(crate) cards: Cards,
    pub(crate) frost: DesktopFrost,
}

pub(crate) struct Space {
    pub(crate) number: usize,
    pub(crate) panes: Vec<Pane>,
}

pub(super) struct Pane {
    pub(crate) id: Arc<str>,
    pub(crate) real: Rect,
    pub(crate) minimized: bool,
}

pub(crate) struct Backdrop {
    pub(crate) wallpaper: Option<ImageSource>,
    pub(crate) taskbar: Option<Shape>,
}

pub(crate) struct Deck {
    board: Board,
    desks: Vec<Element>,
}

#[must_use]
pub(crate) fn deck(board: Board) -> Deck {
    Deck {
        board,
        desks: Vec::new(),
    }
}

impl Deck {
    #[must_use]
    pub(crate) fn holding(mut self, desks: Vec<Element>) -> Deck {
        self.desks = desks;
        self
    }
}

#[derive(Debug, Default)]
pub(crate) struct DeckState {
    born: bool,
    listed: Vec<usize>,
    homes: HashMap<Arc<str>, usize>,
    laid: HashMap<usize, DeckLayout>,
    hits: Hits,
    focus: f32,
    spread: f32,
    settled: bool,
    flung: bool,
    thrown: Option<u64>,
    closing: Option<u64>,
    followed: Option<(u64, f32)>,
    passed: Option<u64>,
    aim: Option<Aim>,
    over: Option<Spot>,
    pointer: Option<Point>,
    press: Option<Press>,
    drag: Option<Drag>,
    dropped: Option<Dropped>,
    scrolling: Option<Scrolling>,
    wheeled: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Aim {
    at: usize,
    showing: usize,
}

#[derive(Debug, Default)]
struct Hits {
    windows: Vec<Hit>,
    closes: Vec<(Arc<str>, Rect)>,
    cards: Vec<(usize, Rect)>,
    thumbs: Vec<(usize, Rect)>,
    pitch: f32,
}

#[derive(Clone, Debug)]
struct Hit {
    id: Arc<str>,
    number: usize,
    rect: Rect,
}

impl Widget for Deck {
    type State = DeckState;

    fn layout(&self) -> Layout {
        Layout::row().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, state: &mut DeckState, ctx: &mut PaintCtx<'_>) {
        self.draw(state, ctx);
    }

    fn event(&self, state: &mut DeckState, event: &Event, ctx: &mut EventCtx<'_>) {
        self.respond(state, event, ctx);
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS | Behavior::TAKES_TAB
    }

    fn debug_name(&self) -> &'static str {
        "OverviewDeck"
    }
}

impl Deck {
    fn showing_number(&self) -> usize {
        self.board
            .spaces
            .get(self.board.showing)
            .map_or(1, |space| space.number)
    }
}

impl std::fmt::Debug for Deck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deck")
            .field("spaces", &self.board.spaces.len())
            .field("showing", &self.board.showing)
            .finish_non_exhaustive()
    }
}

impl View for Deck {
    fn build(mut self) -> Element {
        let desks = std::mem::take(&mut self.desks);
        Element::with_children(self, desks)
    }
}
