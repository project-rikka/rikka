mod flight;
mod swipe;
mod velocity;

use std::time::{Duration, Instant};

use rikka_types::{Point, PointerButton, Rect, Size};
use rikka_ui::{
    Action, Align, Behavior, DurationToken, Element, Event, EventCtx, IntoHandler, Layout,
    PaintCtx, ScrollPhase, ScrollSource, Style, Styled, View, Widget, motion,
};

use crate::blocks::dismissible::flight::Flight;
use crate::blocks::dismissible::swipe::{Release, Step, Swipe};
use crate::blocks::flex::column_of;
use crate::lasting::lasting;

const OFFSET: &str = "offset";

const REACH: &str = "reach";

const SLIDE_OUT: Duration = lasting(motion::SLIDE_OUT);

const SETTLING: DurationToken = DurationToken::Short1;

#[must_use]
pub struct Dismissible {
    child: Option<Element>,
    dismissed: bool,
    arriving: bool,
    on_dismiss: Action,
    style: Style,
}

pub fn dismissible(child: impl View) -> Dismissible {
    Dismissible {
        child: Some(child.build()),
        dismissed: false,
        arriving: false,
        on_dismiss: Action::default(),
        style: Style::default(),
    }
}

impl Dismissible {
    pub const DEPARTURE: Duration = SLIDE_OUT
        .saturating_add(lasting(motion::COLLAPSE))
        .saturating_add(SETTLING.duration());

    pub fn dismissed(mut self, dismissed: bool) -> Self {
        self.dismissed = dismissed;
        self
    }

    pub fn arriving(mut self, arriving: bool) -> Self {
        self.arriving = arriving;
        self
    }

    pub fn on_dismiss<Marker>(mut self, on_dismiss: impl IntoHandler<(), (), Marker>) -> Self {
        self.on_dismiss = Action::new(on_dismiss);
        self
    }

    fn lift(&self, state: &mut DismissibleState, ctx: &mut EventCtx<'_>) {
        if state.swipe.as_ref().is_some_and(Swipe::is_touchpad) {
            return;
        }
        let Some(swipe) = state.swipe.take() else {
            return;
        };
        if !swipe.is_claimed() {
            return;
        }
        ctx.release_pointer();
        self.finish(state, &swipe, ctx);
    }

    fn stroke(
        &self,
        state: &mut DismissibleState,
        delta: Point,
        phase: ScrollPhase,
        ctx: &mut EventCtx<'_>,
    ) {
        let pointing = state
            .swipe
            .as_ref()
            .is_some_and(|swipe| swipe.is_claimed() && !swipe.is_touchpad());
        if pointing {
            return;
        }
        if phase == ScrollPhase::Ended {
            if let Some(swipe) = state.swipe.take().filter(Swipe::is_claimed) {
                self.finish(state, &swipe, ctx);
            }
            return;
        }
        let swipe = match state.swipe.as_mut() {
            Some(swipe) if swipe.is_touchpad() && phase == ScrollPhase::Moved => swipe,
            _ => state.swipe.insert(Swipe::stroking()),
        };
        let step = swipe.stroked(delta, Instant::now());
        follow(step, ctx);
    }

    fn finish(&self, state: &mut DismissibleState, swipe: &Swipe, ctx: &mut EventCtx<'_>) {
        ctx.set_handled();
        ctx.request_paint();
        let release = swipe.release(Instant::now(), ctx.bounds().width());
        state.release = Some(release);
        if release.away {
            ctx.act(&self.on_dismiss, ());
        }
    }

    fn slide(&self, state: &mut DismissibleState, ctx: &mut PaintCtx<'_>, away: f32) -> f32 {
        if self.dismissed {
            return self.depart(state, ctx, away);
        }
        state.departed = false;
        state.flight = None;
        if let Some(swipe) = state.swipe.as_ref().filter(|swipe| swipe.is_claimed()) {
            let along = swipe.along();
            ctx.jump(OFFSET, along);
            return along;
        }
        if let Some(release) = state.release.take() {
            ctx.fling(OFFSET, release.along, release.velocity);
        }
        ctx.animate_curve(OFFSET, 0.0, motion::SLIDE_HOME)
    }

    fn depart(&self, state: &mut DismissibleState, ctx: &mut PaintCtx<'_>, away: f32) -> f32 {
        state.swipe = None;
        if let Some(release) = state.release.take()
            && release.away
        {
            state.flight = Some(Flight::new(
                release.along,
                release.velocity,
                away,
                SLIDE_OUT,
            ));
        }
        let offset = match state.flight.as_mut() {
            Some(flight) => {
                let offset = flight.at(ctx.now(), away);
                ctx.jump(OFFSET, offset);
                if offset < away {
                    ctx.request_frame();
                }
                offset
            }
            None => ctx.animate_curve(OFFSET, away, motion::SLIDE_OUT),
        };
        state.departed |= offset >= away;
        offset
    }
}

fn follow(step: Step, ctx: &mut EventCtx<'_>) {
    if step == Step::Undecided {
        return;
    }
    ctx.set_handled();
    ctx.request_paint();
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Debut {
    #[default]
    Unseen,
    Held,
    Settled,
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct DismissibleState {
    debut: Debut,
    full: f32,
    shown: Option<f32>,
    offset: f32,
    swipe: Option<Swipe>,
    release: Option<Release>,
    flight: Option<Flight>,
    departed: bool,
}

impl DismissibleState {
    fn fit(&mut self, ctx: &mut PaintCtx<'_>, reach: f32) {
        let height = (reach < 1.0).then(|| ctx.snap(self.full * reach.max(0.0)));
        if height != self.shown {
            self.shown = height;
            ctx.request_layout();
        }
    }
}

impl Widget for Dismissible {
    type State = DismissibleState;

    fn layout(&self) -> Layout {
        Layout::row().align_items(Align::Start).with(&self.style)
    }

    fn settled_height(&self, state: &DismissibleState) -> Option<f32> {
        match (state.debut, self.arriving) {
            (Debut::Unseen, true) => Some(0.0),
            _ => state.shown,
        }
    }

    fn behavior(&self, _state: &DismissibleState) -> Behavior {
        let handling = match self.dismissed {
            true => Behavior::INERT,
            false => Behavior::INTERCEPTS,
        };
        handling | Behavior::TRAVELS
    }

    fn event(&self, state: &mut DismissibleState, event: &Event, ctx: &mut EventCtx<'_>) {
        if self.dismissed {
            return;
        }
        match event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                position,
                ..
            } => {
                if state.swipe.as_ref().is_none_or(|swipe| !swipe.is_claimed()) {
                    state.swipe = Some(Swipe::pressed(*position));
                }
            }
            Event::PointerMoved(position) => {
                let Some(swipe) = state.swipe.as_mut() else {
                    return;
                };
                let step = swipe.moved(*position, Instant::now());
                if step == Step::Claimed {
                    ctx.capture_pointer();
                }
                follow(step, ctx);
            }
            Event::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => self.lift(state, ctx),
            Event::PointerExited => {
                if state
                    .swipe
                    .as_ref()
                    .is_some_and(|swipe| !swipe.is_claimed())
                {
                    state.swipe = None;
                }
            }
            Event::PointerCancelled => {
                if let Some(swipe) = state.swipe.take().filter(Swipe::is_claimed) {
                    state.release = Some(Release {
                        along: swipe.along(),
                        velocity: 0.0,
                        away: false,
                    });
                    ctx.request_paint();
                }
            }
            Event::Scroll {
                delta,
                source: ScrollSource::Continuous(phase),
                ..
            } => self.stroke(state, delta.content(), *phase, ctx),
            _ => {}
        }
    }

    fn paint(&self, state: &mut DismissibleState, ctx: &mut PaintCtx<'_>) {
        if let Some(child) = ctx.child_rect(0) {
            state.full = child.height();
        }
        let bounds = ctx.bounds();
        let away = (ctx.viewport().width - bounds.x()).max(bounds.width());
        match state.debut {
            Debut::Unseen => {
                let (offset, reach) = match self.arriving {
                    true => (away, 0.0),
                    false => (0.0, 1.0),
                };
                ctx.jump(OFFSET, offset);
                ctx.jump(REACH, reach);
                state.offset = offset;
                state.debut = match self.arriving {
                    true => Debut::Held,
                    false => Debut::Settled,
                };
                if self.arriving {
                    ctx.request_frame();
                }
                state.fit(ctx, reach);
                return;
            }
            Debut::Held => {
                ctx.jump(OFFSET, away);
                ctx.jump(REACH, 0.0);
                state.debut = Debut::Settled;
            }
            Debut::Settled => {}
        }
        state.offset = self.slide(state, ctx, away);
        let (target, curve) = match state.departed {
            true => (0.0, motion::COLLAPSE),
            false => (1.0, motion::ARRIVAL),
        };
        let reach = ctx.animate_curve(REACH, target, curve);
        state.fit(ctx, reach);
    }

    fn children_offset(&self, state: &DismissibleState, _bounds: Rect, _content: Size) -> Point {
        Point::new(state.offset, 0.0)
    }

    fn same(&self, other: &Self) -> bool {
        self.dismissed == other.dismissed
            && self.arriving == other.arriving
            && self.on_dismiss == other.on_dismiss
            && self.style == other.style
    }
}

impl View for Dismissible {
    fn build(mut self) -> Element {
        let children = self
            .child
            .take()
            .map(|child| {
                column_of([child])
                    .align_items(Align::Stretch)
                    .grow(1.0)
                    .build()
            })
            .into_iter()
            .collect();
        Element::with_children(self, children)
    }
}

impl Styled for Dismissible {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl std::fmt::Debug for Dismissible {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dismissible")
            .field("dismissed", &self.dismissed)
            .field("arriving", &self.arriving)
            .finish_non_exhaustive()
    }
}
