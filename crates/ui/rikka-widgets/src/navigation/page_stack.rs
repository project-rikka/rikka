use std::cell::RefCell;
use std::hash::Hash;
use std::time::{Duration, Instant};

use rikka_animation::Curve;
use rikka_types::{Point, Rect, Size};
use rikka_ui::{
    Behavior, Element, Env, Layout, Length, PaintCtx, Style, Styled, View, Widget, hash_of, motion,
};

use crate::lasting::lasting;

const TRAVEL: f32 = 28.0;

const RISE: f32 = 16.0;

const AT_ONCE: usize = 3;

const HERE: &str = "here";

const SHIFT: &str = "shift";

const GOES: Duration = lasting(motion::TURN_OUT);

const TURNED: Entrance = Entrance {
    curve: motion::TURN_IN,
    delay: motion::TURN_IN_DELAY.duration(),
    takes: motion::TURN_IN_DELAY
        .duration()
        .saturating_add(lasting(motion::TURN_IN)),
};

const ROSE: Entrance = Entrance {
    curve: motion::ARRIVAL,
    delay: Duration::ZERO,
    takes: lasting(motion::ARRIVAL),
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entrance {
    curve: Curve,
    delay: Duration,
    takes: Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageKey {
    outer: usize,
    inner: usize,
}

impl From<usize> for PageKey {
    fn from(at: usize) -> Self {
        PageKey {
            outer: at,
            inner: 0,
        }
    }
}

impl From<(usize, usize)> for PageKey {
    fn from((outer, inner): (usize, usize)) -> Self {
        PageKey { outer, inner }
    }
}

#[must_use]
#[derive(Widget)]
pub struct PageStack {
    #[debug]
    at: PageKey,
    key: Option<u64>,
    content: RefCell<Option<Element>>,
    #[style]
    style: Style,
}

pub fn page_stack(at: impl Into<PageKey>, content: impl View) -> PageStack {
    PageStack {
        at: at.into(),
        key: None,
        content: RefCell::new(Some(content.build())),
        style: Style::default(),
    }
}

impl PageStack {
    pub fn named(mut self, key: impl Hash) -> Self {
        self.key = Some(hash_of(key));
        self
    }

    fn is_another(&self, current_page: CurrentPage) -> bool {
        match self.key {
            Some(key) => Some(key) != current_page.key,
            None => self.at != current_page.at,
        }
    }

    fn turn(&self, state: &mut PageStackState, continuing: bool) {
        if let Some(standing_page) = state.current_page.as_mut()
            && !self.is_another(*standing_page)
        {
            standing_page.at = self.at;
            return;
        }
        let current_page = state.current_page;
        let toward = current_page.map(|current_page| match self.at < current_page.at {
            true => -1.0,
            false => 1.0,
        });
        if let (Some(current_page), Some(toward)) = (current_page, toward)
            && state.room != Size::ZERO
        {
            state.leaving.push(OutgoingPage {
                turn: current_page.turn,
                away: Point::new(-toward * TRAVEL, 0.0),
                room: state.room,
                since: None,
            });
            while state.leaving.len() > AT_ONCE {
                state.leaving.remove(0);
            }
        }
        let (away, enter) = match (toward, continuing) {
            (Some(toward), _) => (Point::new(toward * TRAVEL, 0.0), Some(TURNED)),
            (None, true) => (Point::ZERO, None),
            (None, false) => (Point::new(0.0, RISE), Some(ROSE)),
        };
        state.turns += 1;
        state.current_page = Some(CurrentPage {
            at: self.at,
            key: self.key,
            turn: state.turns,
            away,
            enter,
            since: None,
        });
    }
}

#[derive(Clone, Copy, Debug)]
struct CurrentPage {
    at: PageKey,
    key: Option<u64>,
    turn: u64,
    away: Point,
    enter: Option<Entrance>,
    since: Option<Instant>,
}

#[derive(Clone, Copy, Debug)]
struct OutgoingPage {
    turn: u64,
    away: Point,
    room: Size,
    since: Option<Instant>,
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct PageStackState {
    current_page: Option<CurrentPage>,
    leaving: Vec<OutgoingPage>,
    turns: u64,
    room: Size,
}

impl PageStackState {
    fn arriving(&mut self, now: Instant) -> bool {
        let Some(current_page) = self.current_page.as_mut() else {
            return false;
        };
        let Some(enter) = current_page.enter else {
            return false;
        };
        let since = *current_page.since.get_or_insert(now);
        if now.saturating_duration_since(since) < enter.takes {
            return true;
        }
        current_page.enter = None;
        false
    }

    fn settle(&mut self, now: Instant) -> bool {
        let leaving = self.leaving.len();
        self.leaving.retain_mut(|outgoing| {
            now.saturating_duration_since(*outgoing.since.get_or_insert(now)) < GOES
        });
        self.leaving.len() != leaving
    }

    fn due(&self) -> Option<Instant> {
        let entered = self
            .current_page
            .and_then(|current_page| Some(current_page.since? + current_page.enter?.takes));
        let outgoing = self
            .leaving
            .iter()
            .filter_map(|outgoing| Some(outgoing.since? + GOES));
        entered.into_iter().chain(outgoing).min()
    }

    fn lane(&self, bounds: Rect) -> Rect {
        let tallest = self
            .leaving
            .iter()
            .fold(bounds.height(), |tallest, outgoing| {
                tallest.max(outgoing.room.height)
            });
        Rect::new(bounds.x(), bounds.y(), bounds.width(), tallest)
    }
}

impl Widget for PageStack {
    type State = PageStackState;

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn build(&self, state: &mut PageStackState) -> Vec<Element> {
        let content = self.content.borrow_mut().take();
        self.turn(state, rikka_ui::env().carried);
        let current_page = state.current_page.expect("page stack has a current page");
        let mut pages: Vec<Element> = state
            .leaving
            .iter()
            .map(|outgoing| {
                Element::new(Page::outgoing(outgoing.away, outgoing.room)).keyed(outgoing.turn)
            })
            .collect();
        pages.push(Element::new(Page::here(content, current_page)).keyed(current_page.turn));
        pages
    }

    fn paint(&self, state: &mut PageStackState, ctx: &mut PaintCtx<'_>) {
        if let Some(room) = ctx.child_rect(state.leaving.len()) {
            state.room = room.size;
        }
        let now = ctx.now();
        let arriving = state.arriving(now);
        if state.settle(now) {
            ctx.request_build();
        }
        if let Some(due) = state.due() {
            ctx.wake_at(due);
        }
        if arriving || !state.leaving.is_empty() {
            ctx.set_children_clip(state.lane(ctx.bounds()));
        }
    }
}

struct Page {
    content: RefCell<Option<Element>>,
    away: Point,
    room: Option<Size>,
    enter: Option<Entrance>,
}

impl Page {
    fn here(content: Option<Element>, current_page: CurrentPage) -> Self {
        Page {
            content: RefCell::new(content),
            away: current_page.away,
            room: None,
            enter: current_page.enter,
        }
    }

    fn outgoing(away: Point, room: Size) -> Self {
        Page {
            content: RefCell::new(None),
            away,
            room: Some(room),
            enter: None,
        }
    }
}

#[derive(Debug, Default)]
struct PageState {
    since: Option<Instant>,
    shift: Point,
}

impl Widget for Page {
    type State = PageState;

    fn layout(&self) -> Layout {
        let room = Layout::row().min_height(Length::px(0.0));
        match self.room {
            Some(size) => room
                .size(Length::px(size.width), Length::px(size.height))
                .placed_at(Point::ZERO),
            None => room.grow(1.0),
        }
    }

    fn keeps_children(&self) -> bool {
        self.content.borrow().is_none()
    }

    fn freezes_children(&self) -> bool {
        self.room.is_some()
    }

    fn build(&self, _state: &mut PageState) -> Vec<Element> {
        let mut page = self.content.borrow_mut().take();
        if let Some(page) = page.as_mut() {
            page.style_mut().default_grow.get_or_insert(1.0);
        }
        page.into_iter().collect()
    }

    fn behavior(&self, _state: &PageState) -> Behavior {
        Behavior::INERT.when(self.room.is_some())
    }

    fn surroundings(&self, env: &mut Env) {
        env.carried = true;
    }

    fn paint(&self, state: &mut PageState, ctx: &mut PaintCtx<'_>) {
        let leaving = self.room.is_some();
        let Entrance { curve, delay, .. } = self.enter.unwrap_or(Entrance {
            delay: Duration::ZERO,
            ..TURNED
        });
        let since = *state.since.get_or_insert(ctx.now());
        let waiting = !leaving && ctx.now().saturating_duration_since(since) < delay;
        if waiting {
            ctx.request_frame();
        }
        let settled = !leaving && !waiting;

        if self.enter.is_none() && !ctx.is_animating(HERE) {
            ctx.jump(HERE, 1.0);
        }
        let here = ctx.progress(HERE, settled, curve, motion::TURN_OUT);
        if here < 1.0 {
            ctx.set_subtree_opacity(here);
        }

        if !ctx.is_animating(SHIFT) {
            ctx.jump(SHIFT, self.away);
        }
        let (target_shift, curve) = match (leaving, settled) {
            (true, _) => (self.away, motion::TURN_OUT),
            (false, true) => (Point::ZERO, curve),
            (false, false) => (self.away, curve),
        };
        state.shift = ctx.animate_curve(SHIFT, target_shift, curve);
    }

    fn children_offset(&self, state: &PageState, _bounds: Rect, _content: Size) -> Point {
        state.shift
    }
}
