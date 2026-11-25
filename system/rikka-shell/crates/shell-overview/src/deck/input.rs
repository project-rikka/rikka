use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_types::{Key, Modifiers, Point, PointerButton, Rect, ScrollDelta};
use rikka_ui::{DRAG_SLOP, Event, EventCtx, ScrollPhase, ScrollSource};

use super::{Deck, DeckState, Hit, Lead};
use crate::stage::Stage;
use crate::swipe::{self, Sampler};

const WHEEL_REST: Duration = Duration::from_millis(160);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Spot {
    Window(Arc<str>),
    Card(usize),
    Thumb(usize),
}

#[derive(Clone, Debug)]
pub(super) struct Press {
    spot: Option<Spot>,
    at: Point,
}

#[derive(Clone, Debug)]
pub(super) struct Drag {
    pub(super) id: Arc<str>,
    pub(super) number: usize,
    pub(super) from: Rect,
    pub(super) grab: Point,
    pub(super) at: Point,
    pub(super) rect: Rect,
    pub(super) fresh: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Dropped {
    pub(super) id: Arc<str>,
    pub(super) rect: Rect,
    pub(super) to: Option<usize>,
    pub(super) at: Instant,
}

#[derive(Debug)]
pub(super) struct Scrolling {
    pub(super) start: f32,
    pub(super) travel: f32,
    began: Instant,
    samples: Sampler,
}

impl Scrolling {
    fn new(start: f32) -> Scrolling {
        Scrolling {
            start,
            travel: 0.0,
            began: Instant::now(),
            samples: Sampler::default(),
        }
    }

    fn moved(&mut self, along: f32) {
        self.travel += along;
        let millis = self.began.elapsed().as_millis() as u32;
        self.samples.push(millis, along);
    }

    fn step(&self, pitch: f32) -> i32 {
        let pitch = pitch.max(1.0);
        swipe::step(self.travel / pitch, self.samples.speed() / pitch)
    }
}

impl Deck {
    pub(super) fn respond(&self, state: &mut DeckState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerPressed {
                position,
                button: PointerButton::Primary,
                ..
            } => {
                ctx.set_handled();
                ctx.request_focus();
                state.press = Some(Press {
                    spot: self.spot_at(state, *position),
                    at: *position,
                });
            }
            Event::PointerPressed {
                position,
                button: PointerButton::Middle,
                ..
            } => {
                ctx.set_handled();
                if let Some(Spot::Window(id)) = self.spot_at(state, *position) {
                    ctx.run(move |stage: &mut Stage| stage.close(Arc::clone(&id)));
                }
            }
            Event::PointerMoved(position) => self.moved(state, *position, ctx),
            Event::PointerReleased {
                position,
                button: PointerButton::Primary,
                ..
            } => self.released(state, *position, ctx),
            Event::PointerLeft | Event::PointerExited => {
                state.over = None;
                state.pointer = None;
                ctx.request_paint();
            }
            Event::Scroll { delta, source, .. } => {
                ctx.set_handled();
                self.scrolled(state, *delta, *source, ctx);
            }
            Event::KeyPressed { key, modifiers, .. } => {
                if self.pressed(state, *key, *modifiers, ctx) {
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn moved(&self, state: &mut DeckState, position: Point, ctx: &mut EventCtx<'_>) {
        let led = state.pointer != Some(position) && state.lead != Lead::Pointer;
        if led {
            state.lead = Lead::Pointer;
        }
        state.pointer = Some(position);
        let over = self.spot_at(state, position);
        let changed = led || state.over != over;
        state.over = over;
        if let Some(drag) = &mut state.drag {
            drag.at = position;
            return ctx.request_paint();
        }
        if let Some(press) = &state.press
            && state.settled
            && press.at.distance(position) >= DRAG_SLOP
            && let Some(Spot::Window(id)) = &press.spot
            && let Some(hit) = state.hits.windows.iter().find(|hit| hit.id == *id)
        {
            let rect = hit.rect;
            state.drag = Some(Drag {
                id: Arc::clone(id),
                number: hit.number,
                from: rect,
                grab: Point::new(
                    ((press.at.x - rect.min_x()) / rect.width().max(1.0)).clamp(0.0, 1.0),
                    ((press.at.y - rect.min_y()) / rect.height().max(1.0)).clamp(0.0, 1.0),
                ),
                at: position,
                rect,
                fresh: true,
            });
            ctx.capture_pointer();
            return ctx.request_paint();
        }
        if changed {
            ctx.request_paint();
        }
    }

    fn released(&self, state: &mut DeckState, position: Point, ctx: &mut EventCtx<'_>) {
        ctx.set_handled();
        let press = state.press.take();
        if let Some(drag) = state.drag.take() {
            ctx.release_pointer();
            ctx.request_paint();
            let to = match self.spot_at(state, position) {
                Some(Spot::Card(number) | Spot::Thumb(number)) if number != drag.number => {
                    Some(number)
                }
                _ => None,
            };
            state.dropped = Some(Dropped {
                id: Arc::clone(&drag.id),
                rect: drag.rect,
                to,
                at: Instant::now(),
            });
            if let Some(number) = to {
                ctx.run(move |stage: &mut Stage| stage.send(Arc::clone(&drag.id), number));
            }
            return;
        }
        let Some(press) = press.filter(|press| press.spot == self.spot_at(state, position)) else {
            return;
        };
        match press.spot {
            None => {
                ctx.run(Stage::shut);
            }
            Some(Spot::Window(id)) => {
                ctx.run(move |stage: &mut Stage| stage.activate(Arc::clone(&id)));
            }
            Some(Spot::Card(number)) if number == self.showing_number() => {
                ctx.run(Stage::shut);
            }
            Some(Spot::Card(number) | Spot::Thumb(number)) => self.show(state, number, ctx),
        }
    }

    fn scrolled(
        &self,
        state: &mut DeckState,
        delta: ScrollDelta,
        source: ScrollSource,
        ctx: &mut EventCtx<'_>,
    ) {
        let offset = delta.offset();
        match source {
            ScrollSource::Wheel => {
                let along = match offset.y.abs() >= offset.x.abs() {
                    true => offset.y,
                    false => offset.x,
                };
                let rested = state.wheeled.is_none_or(|at| at.elapsed() >= WHEEL_REST);
                if along != 0.0 && rested {
                    state.wheeled = Some(Instant::now());
                    self.step(state, along.signum() as i32, ctx);
                }
            }
            ScrollSource::Continuous(phase) => {
                if phase == ScrollPhase::Began || state.scrolling.is_none() {
                    state.scrolling = Some(Scrolling::new(state.focus));
                }
                if let Some(scrolling) = &mut state.scrolling {
                    scrolling.moved(offset.x);
                }
                if phase == ScrollPhase::Ended
                    && let Some(scrolling) = state.scrolling.take()
                {
                    let step = scrolling.step(state.hits.pitch);
                    self.step(state, step, ctx);
                }
                ctx.request_paint();
            }
        }
    }

    fn pressed(
        &self,
        state: &mut DeckState,
        key: Key,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let chosen = self.lifted(state).or(self.board.selected.as_ref()).cloned();
        match key {
            Key::Escape => match state.drag.take() {
                Some(drag) => {
                    ctx.release_pointer();
                    state.dropped = Some(Dropped {
                        id: drag.id,
                        rect: drag.rect,
                        to: None,
                        at: Instant::now(),
                    });
                    ctx.request_paint();
                }
                None => {
                    ctx.run(Stage::shut);
                }
            },
            Key::Enter | Key::Space => {
                if let Some(id) = chosen {
                    ctx.run(move |stage: &mut Stage| stage.activate(Arc::clone(&id)));
                }
            }
            Key::Delete => {
                if let Some(id) = chosen {
                    ctx.run(move |stage: &mut Stage| stage.close(Arc::clone(&id)));
                }
            }
            Key::PageUp => self.step(state, -1, ctx),
            Key::PageDown => self.step(state, 1, ctx),
            Key::ArrowLeft if modifiers.control => self.step(state, -1, ctx),
            Key::ArrowRight if modifiers.control => self.step(state, 1, ctx),
            Key::Tab => self.cycle(state, if modifiers.shift { -1 } else { 1 }, ctx),
            Key::ArrowLeft => self.walk(state, Point::new(-1.0, 0.0), ctx),
            Key::ArrowRight => self.walk(state, Point::new(1.0, 0.0), ctx),
            Key::ArrowUp => self.walk(state, Point::new(0.0, -1.0), ctx),
            Key::ArrowDown => self.walk(state, Point::new(0.0, 1.0), ctx),
            Key::Char(digit) => match workspace_of(digit) {
                Some(number) => self.show(state, number, ctx),
                None => return false,
            },
            _ => return false,
        }
        true
    }

    fn step(&self, state: &mut DeckState, step: i32, ctx: &mut EventCtx<'_>) {
        if step == 0 {
            return;
        }
        self.aim(state, step);
        ctx.run(move |stage: &mut Stage| stage.step(step));
        ctx.request_paint();
    }

    fn show(&self, state: &mut DeckState, number: usize, ctx: &mut EventCtx<'_>) {
        if let Some(at) = self
            .board
            .spaces
            .iter()
            .position(|space| space.number == number)
        {
            self.aim(state, at as i32 - self.board.showing as i32);
        }
        ctx.run(move |stage: &mut Stage| stage.show(number));
        ctx.request_paint();
    }

    fn cycle(&self, state: &mut DeckState, by: i32, ctx: &mut EventCtx<'_>) {
        let Some(from) = self.navigated_from(state, ctx) else {
            return;
        };
        let windows = &state.hits.windows;
        let at = windows
            .iter()
            .position(|hit| hit.id == from.id)
            .unwrap_or(0) as i32;
        let next = (at + by).rem_euclid(windows.len() as i32) as usize;
        select(&windows[next], ctx);
    }

    fn walk(&self, state: &mut DeckState, toward: Point, ctx: &mut EventCtx<'_>) {
        let Some(from) = self.navigated_from(state, ctx) else {
            return;
        };
        let origin = from.rect.center();
        let nearest = state
            .hits
            .windows
            .iter()
            .filter(|hit| hit.id != from.id)
            .filter_map(|hit| {
                let center = hit.rect.center();
                let ahead = (center.x - origin.x) * toward.x + (center.y - origin.y) * toward.y;
                let aside = (center.x - origin.x) * toward.y - (center.y - origin.y) * toward.x;
                (ahead > 0.0).then(|| (hit, ahead + aside.abs() * 2.0))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b));
        if let Some((hit, _)) = nearest {
            select(hit, ctx);
        }
    }

    fn navigated_from(&self, state: &mut DeckState, ctx: &mut EventCtx<'_>) -> Option<Hit> {
        let windows = &state.hits.windows;
        let from = self
            .lifted(state)
            .or(self.board.selected.as_ref())
            .and_then(|id| windows.iter().find(|hit| hit.id == *id))
            .or_else(|| windows.first())?
            .clone();
        if std::mem::replace(&mut state.lead, Lead::Keyboard) == Lead::Keyboard {
            return Some(from);
        }
        select(&from, ctx);
        None
    }

    fn spot_at(&self, state: &DeckState, point: Point) -> Option<Spot> {
        let hits = &state.hits;
        if let Some(hit) = hits.windows.iter().rev().find(|hit| {
            hit.rect.contains(point) && state.drag.as_ref().is_none_or(|drag| drag.id != hit.id)
        }) {
            return Some(Spot::Window(Arc::clone(&hit.id)));
        }
        if let Some((number, _)) = hits.thumbs.iter().find(|(_, rect)| rect.contains(point)) {
            return Some(Spot::Thumb(*number));
        }
        hits.cards
            .iter()
            .rev()
            .find(|(_, rect)| rect.contains(point))
            .map(|(number, _)| Spot::Card(*number))
    }
}

fn select(hit: &Hit, ctx: &mut EventCtx<'_>) {
    let id = Arc::clone(&hit.id);
    ctx.run(move |stage: &mut Stage| stage.select(Arc::clone(&id)));
    ctx.request_paint();
}

fn workspace_of(digit: char) -> Option<usize> {
    match digit {
        '0' => Some(10),
        '1'..='9' => digit.to_digit(10).map(|number| number as usize),
        _ => None,
    }
}
