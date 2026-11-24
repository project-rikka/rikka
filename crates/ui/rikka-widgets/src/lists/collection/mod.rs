mod ahead;
mod cells;
pub(crate) mod highlight;
pub(crate) mod lines;
mod ride;
mod rider;
mod shuffle;
mod spans;
mod walk;

use rikka_ui::{
    Action, Arranged, Axis, Behavior, Element, Event, EventCtx, Inset, Layout, Length, PaintCtx,
    Reveal, ScrollAnchor, Style, Version, View, Widget, hash_of,
};
use rustc_hash::FxHashSet;
use std::ops::Range;
use std::rc::Rc;
use std::time::Instant;

use rikka_types::{Key, Modifiers, Point, PointerButton, Rect, Size};

use crate::blocks::animated_position::AnimatedPosition;
use ahead::{Ahead, Heading};
use cells::{Cells, Keys};
use ride::{DriftState, Ride, Riders};
use rider::Rider;
use rikka_ui::scroll::{Place, ScrollPosition, Viewing, bar};
use shuffle::{Shuffle, Stage};
use std::sync::atomic::{AtomicU64, Ordering};

pub use highlight::Highlights;
pub(crate) use lines::Lines;
pub(crate) use spans::Spans;

pub(crate) use walk::KeyboardWalk;

pub use lines::Surface;

const ESTIMATE: f32 = 160.0;

const OPENING: f32 = 3.0;

const RETRIES: u8 = 3;

const SPARE: usize = 2;

const REACH: usize = 3;

const NUDGE: f32 = 0.5;

fn reached(here: Range<usize>, there: Range<usize>) -> Range<usize> {
    let start = here.start.min(there.start);
    let end = here.end.max(there.end);
    match end - start <= there.len().saturating_mul(REACH) {
        true => start..end,
        false => there,
    }
}

type Cell = Rc<dyn Fn(Slot) -> Element>;
type KeyFn = Rc<dyn Fn(usize) -> u64>;
type Memo = Rc<dyn Fn(usize) -> Option<Version>>;
type Reaches = Rc<dyn Fn(usize) -> bool>;
type PointerFn = Rc<dyn Fn(&Event, &mut EventCtx<'_>, &Surface<'_>) -> bool>;

#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub index: usize,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rearrange {
    #[default]
    Ride,
    Glide,
    Settle,
}

pub(crate) struct Props {
    pub(crate) uniform: Option<f32>,
    pub(crate) estimate: Option<f32>,
    pub(crate) gap: f32,
    pub(crate) padding: Inset,
    pub(crate) selected: Option<usize>,
    pub(crate) focus: Option<usize>,
    pub(crate) reveal: Option<usize>,
    pub(crate) follow: bool,
    pub(crate) subject: u64,
    pub(crate) place: Option<Place>,
    pub(crate) named: Option<KeyFn>,
    pub(crate) memo: Option<Memo>,
    pub(crate) reachable: Option<Reaches>,
    pub(crate) on_select: Action<(usize,)>,
    pub(crate) on_activate: Action<(usize,)>,
    pub(crate) on_focus: Action<(usize, Modifiers)>,
    pub(crate) on_key: Action<(Key, Modifiers, Option<usize>), bool>,
    pub(crate) press: Action<(PointerButton, Modifiers)>,
    pub(crate) on_pointer: Option<PointerFn>,
    pub(crate) on_visible: Action<(Range<usize>,)>,
    pub(crate) activates_on_press: bool,
    pub(crate) focusable: bool,
    pub(crate) autofocus: bool,
    pub(crate) takes_tab: bool,
    pub(crate) lit: bool,
    pub(crate) rearrange: Rearrange,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            uniform: None,
            estimate: None,
            gap: 0.0,
            padding: Inset::NONE,
            selected: None,
            focus: None,
            reveal: None,
            follow: false,
            subject: 0,
            place: None,
            named: None,
            memo: None,
            reachable: None,
            on_select: Action::default(),
            on_activate: Action::default(),
            on_focus: Action::default(),
            on_key: Action::default(),
            press: Action::default(),
            on_pointer: None,
            on_visible: Action::default(),
            activates_on_press: false,
            focusable: false,
            autofocus: false,
            takes_tab: false,
            lit: false,
            rearrange: Rearrange::Ride,
        }
    }
}

impl Clone for Props {
    fn clone(&self) -> Self {
        Self {
            place: self.place.clone(),
            named: self.named.clone(),
            memo: self.memo.clone(),
            reachable: self.reachable.clone(),
            on_select: self.on_select.clone(),
            on_activate: self.on_activate.clone(),
            on_focus: self.on_focus.clone(),
            on_key: self.on_key.clone(),
            press: self.press.clone(),
            on_pointer: self.on_pointer.clone(),
            on_visible: self.on_visible.clone(),
            ..*self
        }
    }
}
#[must_use]
#[derive(Widget)]
pub struct Collection {
    #[debug]
    lines: Lines,
    #[debug]
    count: usize,
    cell: Cell,
    made: u64,
    props: Props,
    #[style]
    style: Style,
}

pub fn lazy_column<V: View>(count: usize, cell: impl Fn(Slot) -> V + 'static) -> Collection {
    Collection::new(Lines::Down, count, cell)
}

pub fn lazy_row<V: View>(count: usize, cell: impl Fn(Slot) -> V + 'static) -> Collection {
    Collection::new(Lines::Across, count, cell)
}

pub fn lazy_grid<V: View>(
    columns: usize,
    count: usize,
    cell: impl Fn(Slot) -> V + 'static,
) -> Collection {
    Collection::new(Lines::Grid(columns.max(1)), count, cell)
}

pub fn lazy_tiles<V: View>(
    width: Length,
    count: usize,
    cell: impl Fn(Slot) -> V + 'static,
) -> Collection {
    let lines = match width.fixed() {
        Some(tile) => Lines::Tiles(tile.max(1.0)),
        None => Lines::Down,
    };
    Collection::new(lines, count, cell)
}

impl Collection {
    fn new<V: View>(lines: Lines, count: usize, cell: impl Fn(Slot) -> V + 'static) -> Self {
        Self::from_parts(
            lines,
            count,
            Rc::new(move |slot| cell(slot).build()),
            Props::default(),
            Style::default(),
        )
    }

    pub(crate) fn from_parts(
        lines: Lines,
        count: usize,
        cell: Cell,
        props: Props,
        style: Style,
    ) -> Self {
        static MADE: AtomicU64 = AtomicU64::new(1);
        Self {
            lines,
            count,
            cell,
            made: MADE.fetch_add(1, Ordering::Relaxed),
            props,
            style,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn columns(mut self, columns: usize) -> Self {
        self.lines = Lines::Grid(columns.max(1));
        self
    }

    pub fn tiles(mut self, width: impl Into<Length>) -> Self {
        self.lines = match width.into().fixed() {
            Some(tile) => Lines::Tiles(tile.max(1.0)),
            None => Lines::Down,
        };
        self
    }

    collection_methods!();

    fn guess(&self) -> f32 {
        self.props.estimate.unwrap_or(ESTIMATE).max(1.0)
    }

    fn pitch(&self) -> Option<f32> {
        self.props.uniform.map(|extent| extent + self.props.gap)
    }

    fn selection(&self) -> Option<usize> {
        self.props
            .selected
            .filter(|index| *index < self.count && self.reaches(*index))
    }

    fn selected_index(&self) -> Option<usize> {
        self.props.focus.or(self.selection())
    }

    fn reaches(&self, index: usize) -> bool {
        match &self.props.reachable {
            Some(reaches) => reaches(index),
            None => true,
        }
    }

    fn seat_at(&self, index: usize) -> Seat {
        Seat {
            index,
            name: self.props.named.as_ref().map(|named| named(index)),
        }
    }

    fn seat(&self) -> Option<Seat> {
        Some(self.seat_at(self.props.reveal.or(self.selected_index())?))
    }

    fn along(&self) -> Axis {
        self.lines.along()
    }

    fn corner(&self) -> Point {
        Point::new(self.props.padding.left(), self.props.padding.top())
    }

    fn body(&self, room: Rect) -> Rect {
        Rect::new(
            room.x() + self.props.padding.left(),
            room.y() + self.props.padding.top(),
            (room.width() - self.props.padding.horizontal()).max(0.0),
            (room.height() - self.props.padding.vertical()).max(0.0),
        )
    }

    fn across(&self, state: &CollectionState) -> usize {
        self.lines.across(state.breadth, self.props.gap)
    }

    fn bands(&self, across: usize) -> usize {
        self.count.div_ceil(across.max(1))
    }

    fn cells(&self) -> Cells {
        Cells {
            cell: Rc::clone(&self.cell),
            memo: self.props.memo.clone(),
            subject: self.props.subject,
            chosen: self.selection(),
            made: self.made,
        }
    }

    fn surface<'a>(&self, state: &'a CollectionState, body: Rect) -> Surface<'a> {
        let across = self.across(state).max(1);
        Surface {
            body,
            lines: self.lines,
            offset: state.viewing.scroll.rendered(),
            across,
            breadth: self.lines.breadth(body.width(), across, self.props.gap),
            gap: self.props.gap,
            count: self.count,
            spans: &state.spans,
        }
    }

    fn walked(&self, state: &CollectionState, room: f32) -> KeyboardWalk {
        KeyboardWalk {
            count: self.count,
            columns: match self.lines {
                Lines::Across => self.count.max(1),
                _ => self.across(state),
            },
            page: state.spans.page(room),
        }
    }

    fn aim_at_band(&self, state: &mut CollectionState, band: usize, room: f32) {
        let furthest = state.furthest(room);
        let (start, extent) = (state.spans.top(band), state.spans.height(band));
        state.viewing.aim_at(start, extent, room, furthest);
    }

    fn report(&self, state: &mut CollectionState, ctx: &mut EventCtx<'_>) {
        let Some(on_visible) = self.props.on_visible.handler() else {
            return;
        };
        let across = self.across(state);
        let bands = state
            .spans
            .visible(state.viewing.scroll.target(), state.room);
        let visible = (bands.start * across).min(self.count)..(bands.end * across).min(self.count);
        let visibility = (self.props.subject, visible);
        if state.reported.as_ref() != Some(&visibility) {
            state.reported = Some(visibility.clone());
            ctx.call(on_visible, (visibility.1,));
        }
    }

    fn choose(&self, index: usize, modifiers: Modifiers, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.props.on_focus, (index, modifiers));
        if let Some(on_select) = self.props.on_select.handler()
            && self.selection() != Some(index)
        {
            ctx.call(on_select, (index,));
        }
    }

    fn activate(&self, index: usize, ctx: &mut EventCtx<'_>) {
        ctx.act(&self.props.on_activate, (index,));
    }

    fn pressed(&self, state: &mut CollectionState, event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::PointerPressed {
            button,
            modifiers,
            position,
        } = *event
        else {
            return;
        };
        if !ctx.bounds().contains(position) {
            return;
        }
        ctx.act(&self.props.press, (button, modifiers));
        if button != PointerButton::Primary {
            return;
        }
        ctx.set_handled();
        let body = self.body(ctx.room());
        let Some(index) = self
            .surface(state, body)
            .at(position)
            .filter(|index| self.reaches(*index))
        else {
            return;
        };
        self.choose(index, modifiers, ctx);
        if ctx.presses() == 2 || self.props.activates_on_press {
            self.activate(index, ctx);
        }
    }

    fn keyed(&self, state: &mut CollectionState, event: &Event, ctx: &mut EventCtx<'_>) {
        let Event::KeyPressed { key, modifiers, .. } = *event else {
            return;
        };
        if ctx
            .answer(&self.props.on_key, (key, modifiers, self.selected_index()))
            .unwrap_or(false)
        {
            ctx.set_handled();
            return;
        }
        let walked = self.walked(state, state.room);
        let Some(Some(index)) = walked.to(key, self.selected_index(), |index| self.reaches(index))
        else {
            if matches!(key, Key::Enter | Key::Space)
                && let Some(index) = self.selection()
            {
                ctx.set_handled();
                self.activate(index, ctx);
            }
            return;
        };
        ctx.set_handled();
        self.aim_at_band(state, index / self.across(state).max(1), state.room);
        state.revealed = Some(self.seat_at(index));
        self.choose(index, modifiers, ctx);
        self.report(state, ctx);
        ctx.request_paint();
    }
}

#[derive(Clone, Copy, Debug)]
struct Seat {
    index: usize,
    name: Option<u64>,
}

impl Seat {
    fn moved(self, from: Self) -> bool {
        match (self.name, from.name) {
            (Some(name), Some(was)) => name != was,
            _ => self.index != from.index,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AnchorPoint {
    band: usize,
    screen: f32,
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct CollectionState {
    spans: Spans,
    viewing: Viewing,
    window: Range<usize>,
    across: usize,
    subject: Option<u64>,
    built: Option<u64>,
    breadth: f32,
    room: f32,
    shift: f32,
    laid_top: f32,
    origin: f32,
    anchor: Option<(usize, f32)>,
    pending_anchor: Option<AnchorPoint>,
    retries: u8,
    frame: Option<Instant>,
    placing: bool,
    revealed: Option<Seat>,
    reported: Option<(u64, Range<usize>)>,
    hovered: Option<usize>,
    fills: Highlights,
    reported_content: f32,
    laid: Option<u64>,
    cells: FxHashSet<u64>,
    standing: FxHashSet<u64>,
    arrivals: FxHashSet<u64>,
    names: Vec<u64>,
    previous: Option<Cells>,
    pending: Option<Shuffle>,
    ride: Option<Ride>,
    riders: Riders,
    generation: u64,
    keys: Keys,
    heading: Heading,
}

impl CollectionState {
    fn furthest(&self, room: f32) -> f32 {
        bar::furthest(room, self.spans.content())
    }

    fn adopt(&mut self, place: Option<&Place>, subject: u64) {
        let Some(stand) = self.viewing.adopt(place, Some(subject)) else {
            return;
        };
        if let Some((band, within)) = stand.row {
            let start = self.spans.top(band);
            self.viewing.scroll.snap(start + within);
            self.anchor = Some((band, start));
        }
        self.placing = true;
    }

    fn publish(&mut self, place: Option<&Place>, room: f32, subject: u64) {
        if place.is_none() {
            return;
        }
        let offset = self.viewing.scroll.target();
        let row = self.spans.measures().then(|| {
            let band = self.spans.at(offset);
            (band, offset - self.spans.top(band))
        });
        let stand = ScrollPosition {
            subject: Some(subject),
            row,
            ..self.viewing.scroll.stand(self.furthest(room), room)
        };
        self.viewing.scroll.publish(place, stand);
    }

    fn settle(&mut self) {
        self.pending = None;
        self.ride = None;
        self.riders.clear();
    }

    fn hold(&mut self, point: AnchorPoint) {
        self.pin(self.spans.top(point.band) - point.screen - self.viewing.scroll.rendered());
    }

    fn keep(&mut self, scroll_anchor: ScrollAnchor) {
        let was = self.laid_top + scroll_anchor.was;
        let is = self.spans.top(self.window.start) + scroll_anchor.is;
        self.pin(is - was);
        if scroll_anchor.touched {
            self.viewing.settle(self.furthest(self.room));
        }
    }

    fn pin(&mut self, moved: f32) {
        self.viewing.scroll.shift(moved);
        let offset = self.viewing.scroll.rendered();
        self.shift = self.spans.top(self.window.start) - offset;
        self.anchor_at(offset);
    }

    fn anchor_at(&mut self, offset: f32) {
        let band = self.spans.at(offset);
        self.anchor = Some((band, self.spans.top(band)));
    }
}

impl Widget for Collection {
    type State = CollectionState;

    fn layout(&self) -> Layout {
        Layout::grid(self.lines.tracks(self.props.uniform))
            .scrolling(self.along())
            .padding(self.props.padding)
            .gap_exact(self.props.gap, self.props.gap)
            .size(Length::FILL, Length::HUG)
            .with(&self.style)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::CLIPS
            | Behavior::TRACKS_HOVER
            | Behavior::FOCUSABLE.when(self.props.focusable)
            | Behavior::AUTOFOCUS.when(self.props.autofocus)
            | Behavior::TAKES_TAB.when(self.props.takes_tab)
    }

    fn build(&self, state: &mut CollectionState) -> Vec<Element> {
        let across = self.across(state).max(1);
        let reflowed = std::mem::replace(&mut state.across, across) != across;
        let rearranged = self.carry(state, across, reflowed);
        state
            .spans
            .adopt(self.bands(across), self.guess(), self.pitch());

        let room = match state.room > 0.0 {
            true => state.room,
            false => self.guess() * OPENING,
        };
        state.adopt(self.props.place.as_ref(), self.props.subject);
        let furthest = state.furthest(room);
        if self.props.follow && state.viewing.holds_end() {
            state.viewing.scroll.snap(furthest);
        }
        let standing = state.viewing.scroll.rendered().clamp(0.0, furthest);
        let aimed = state.viewing.scroll.target().clamp(0.0, furthest);
        let visible_range = state.spans.visible_range(aimed, room);
        let window = reached(
            state.spans.visible_range(standing, room),
            visible_range.clone(),
        );
        let built = match state.built == Some(self.props.subject) {
            true => state.window.clone(),
            false => 0..0,
        };
        let window = self.ahead(state, &visible_range, room).extend(
            window,
            &built,
            rikka_ui::prefetch_deadline().is_some(),
        );
        let known = std::mem::replace(&mut state.window, window.clone());
        let known = match state.built.replace(self.props.subject) == Some(self.props.subject) {
            true => known,
            false => 0..0,
        };
        self.choreograph(state);
        self.remember_names(state, across);
        let built = self.cells();
        let cells = (window.start * across)..(window.end * across).min(self.count);
        state.keys.adopt(self.made, cells.clone());
        if !self.rides() {
            return self.gliding(state, &built, cells, known, rearranged, across);
        }
        let mut elements = self.riding(state, &built, cells);
        elements.extend(self.ghosts(state, &built));
        state.previous = Some(built);
        elements
    }

    fn content_length(&self, state: &Self::State) -> Option<f32> {
        Some(state.spans.content())
    }

    fn children_offset(&self, state: &CollectionState, _bounds: Rect, _content: Size) -> Point {
        self.along().point(state.shift, 0.0)
    }

    fn arranged(&self, state: &mut CollectionState, arranged: Arranged<'_>) -> bool {
        let along = self.along();
        let body = self.body(arranged.room());
        let breadth = along.across().room(body);
        let fresh = state.laid.is_some_and(|was| was != self.props.subject);
        let rewidened = (state.breadth - breadth).abs() > NUDGE;
        state.laid = Some(self.props.subject);
        if fresh {
            state.settle();
        }
        if fresh || rewidened {
            state.spans.forget();
        }
        let shed = rewidened && std::mem::take(&mut state.heading) != Heading::Unknown;
        state.breadth = breadth;
        state.room = along.room(body);
        if state.across != self.across(state) {
            state.spans.forget();
            return true;
        }
        let remeasured = state.spans.measures() && self.measure(state, &arranged);
        if let Some(point) = state.pending_anchor.take() {
            state.hold(point);
        } else if let Some(scroll_anchor) = arranged.anchor() {
            state.keep(scroll_anchor);
        }
        state.laid_top = state.spans.top(state.window.start);
        let content = state.spans.content();
        let reported_content = (content - state.reported_content).abs() > NUDGE;
        state.reported_content = content;
        let prefetches = rikka_ui::prefetch_deadline().is_some_and(|until| Instant::now() < until)
            && self.lacks_ahead(state);
        reported_content
            || (remeasured && state.ride.is_some())
            || !self.holds(state)
            || prefetches
            || shed
    }

    fn paint(&self, state: &mut CollectionState, ctx: &mut PaintCtx<'_>) {
        let along = self.along();
        let bounds = ctx.bounds();
        let body = self.body(ctx.room());

        let fresh = state.subject.is_some_and(|was| was != self.props.subject);
        state.subject = Some(self.props.subject);
        if fresh {
            state.anchor = None;
            state.viewing.opened();
            state.placing = true;
        }
        state.adopt(self.props.place.as_ref(), self.props.subject);
        state.pending = None;
        state.arrivals.clear();
        if let Some(ride) = state.ride.as_mut() {
            match state.riders.ride(ride, ctx.now(), &state.spans) {
                true => ctx.request_frame(),
                false => {
                    state.ride = None;
                    ctx.request_build();
                }
            }
        }
        state
            .viewing
            .swallows_next_reveal(std::mem::take(&mut state.placing));
        if state.frame != Some(ctx.now()) {
            state.frame = Some(ctx.now());
            state.retries = 0;
        }

        let revealing = self.seat();
        let reveal_pending = match (revealing, state.revealed) {
            (Some(seat), Some(was)) => seat.moved(was),
            (Some(_), None) => true,
            (None, _) => false,
        };
        state.revealed = revealing;
        if reveal_pending && let Some(seat) = revealing {
            match state.furthest(state.room) > 0.0 {
                true => self.aim_at_band(state, seat.index / state.across.max(1), state.room),
                false => ctx.reveal(self.surface(state, body).cell(seat.index), Reveal::Nearest),
            }
        }

        let anchored = state.anchor.filter(|(band, _)| *band < state.spans.count());
        if let Some((band, was)) = anchored {
            state.viewing.scroll.shift(state.spans.top(band) - was);
        }

        let furthest = state.furthest(state.room);
        state
            .viewing
            .heed(self.props.place.as_ref(), furthest, state.room);
        if self.props.follow {
            state.viewing.follow(furthest, false);
        }

        let offset = state.viewing.scroll.advance(ctx, along, furthest);
        let travel = state.viewing.scroll.target() - offset;
        if travel.abs() > NUDGE {
            state.heading = Heading::from_travel(travel);
        }
        state.publish(self.props.place.as_ref(), state.room, self.props.subject);
        state.shift = ctx.snap(state.spans.top(state.window.start) - offset);
        state.anchor_at(offset);

        if self.props.lit {
            self.light(state, ctx, body, offset);
        }

        state.viewing.paint_bar(
            ctx,
            along,
            bar::track(along, bounds, body),
            state.spans.content(),
            offset,
        );

        let holds = self.holds(state);
        if holds {
            state.retries = 0;
        } else if state.retries < RETRIES {
            state.retries += 1;
            ctx.request_build();
        }
        if self.lacks_ahead(state) {
            ctx.request_prefetch();
        }
    }

    fn reveal(
        &self,
        state: &mut CollectionState,
        room: Rect,
        _content: Size,
        rect: Rect,
        reveal: Reveal,
    ) -> bool {
        let body = self.body(room);
        let furthest = state.furthest(self.along().room(body));
        state
            .viewing
            .reveal(self.along(), body, rect, furthest, reveal)
    }

    fn event(&self, state: &mut CollectionState, event: &Event, ctx: &mut EventCtx<'_>) {
        let along = self.along();
        let body = self.body(ctx.room());
        state.adopt(self.props.place.as_ref(), self.props.subject);
        self.report(state, ctx);

        let furthest = state.furthest(state.room);
        if state.viewing.grabbed(
            along,
            bar::track(along, ctx.bounds(), body),
            state.spans.content(),
            furthest,
            event,
            ctx,
        ) {
            state.retries = 0;
            return;
        }
        if let Some(on_pointer) = &self.props.on_pointer {
            let surface = self.surface(state, body);
            if on_pointer(event, ctx, &surface) {
                return;
            }
        }

        match event {
            Event::PointerPressed { .. } => self.pressed(state, event, ctx),
            Event::KeyPressed { .. } => self.keyed(state, event, ctx),
            Event::PointerMoved(position) if self.props.lit => {
                let under = self
                    .surface(state, body)
                    .at(*position)
                    .filter(|index| self.reaches(*index));
                if state.hovered != under {
                    state.hovered = under;
                    ctx.request_paint();
                }
            }
            Event::PointerExited if state.hovered.is_some() => {
                state.hovered = None;
                ctx.request_paint();
            }
            Event::Scroll { delta, source, .. } => {
                let travel = bar::travel(along, *delta, *source);
                if !state.viewing.nudged(travel, furthest) {
                    return;
                }
                state.heading = Heading::from_travel(travel);
                state.retries = 0;
                state.hovered = None;
                self.report(state, ctx);
                ctx.request_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }
}

impl Collection {
    fn holds(&self, state: &CollectionState) -> bool {
        let visible_range = state
            .spans
            .visible_range(state.viewing.scroll.target(), state.room);
        let allowed = self
            .ahead(state, &visible_range, state.room)
            .around(&visible_range);
        state.window.start <= visible_range.start
            && state.window.end >= visible_range.end
            && state.window.len() <= allowed.len().saturating_mul(SPARE)
    }

    fn lacks_ahead(&self, state: &CollectionState) -> bool {
        let furthest = state.furthest(state.room);
        let aimed = state.viewing.scroll.target().clamp(0.0, furthest);
        let visible_range = state.spans.visible_range(aimed, state.room);
        self.ahead(state, &visible_range, state.room)
            .lacks(&state.window)
    }

    fn ahead(&self, state: &CollectionState, visible_range: &Range<usize>, room: f32) -> Ahead {
        let furthest = state.furthest(room);
        let heading = match state.heading {
            Heading::Forward if bar::at_end(state.viewing.scroll.target(), furthest) => {
                Heading::Back
            }
            heading => heading,
        };
        Ahead::new(&state.spans, visible_range, heading, room)
    }

    fn carry(&self, state: &mut CollectionState, across: usize, reflowed: bool) -> bool {
        let Some(named) = &self.props.named else {
            state.settle();
            return false;
        };
        if reflowed || state.built != Some(self.props.subject) {
            state.settle();
            return false;
        }
        let bands = self.bands(across);
        let name = |band: usize| named(band * across);
        let changed = bands != state.spans.count()
            || state
                .window
                .clone()
                .zip(&state.names)
                .any(|(band, was)| band >= bands || name(band) != *was);
        if !changed {
            return false;
        }
        match state.reshuffle(bands, &name, self.props.follow) {
            Some(shuffle)
                if self.rides()
                    && self.props.rearrange == Rearrange::Ride
                    && !self.props.follow =>
            {
                state.pending = Some(shuffle);
            }
            _ => state.settle(),
        }
        true
    }

    fn choreograph(&self, state: &mut CollectionState) {
        let (Some(shuffle), Some(named)) = (&state.pending, &self.props.named) else {
            return;
        };
        let stage = Stage {
            spans: &state.spans,
            offset: state.viewing.scroll.rendered(),
            room: state.room,
            window: state.window.clone(),
        };
        state.ride = shuffle.ride(&stage, &|index| named(index));
        if state.ride.is_none() {
            state.riders.clear();
        }
    }

    fn rides(&self) -> bool {
        self.props.named.is_some()
            && self.props.rearrange != Rearrange::Glide
            && matches!(self.lines, Lines::Down | Lines::Across)
    }

    fn named_key(&self, name: u64) -> u64 {
        hash_of((self.props.subject, "named", name))
    }

    fn gliding(
        &self,
        state: &mut CollectionState,
        built: &Cells,
        cells: Range<usize>,
        known: Range<usize>,
        rearranged: bool,
        across: usize,
    ) -> Vec<Element> {
        let fades = !self.props.follow;
        let keeps = rikka_ui::can_retain();
        std::mem::swap(&mut state.cells, &mut state.standing);
        state.cells.clear();
        let mut band = (usize::MAX, 0.0);
        cells
            .map(|index| {
                let at = index / across;
                if at != band.0 {
                    band = (at, state.spans.top(at));
                }
                let along = band.1 - state.origin;
                let name = self.props.named.as_ref().map(|named| named(index));
                let place = hash_of((along.to_bits(), name));
                if keeps
                    && let Some(key) = built
                        .retained_key(index, place, &state.keys)
                        .map(|key| name.map_or(key, |name| self.named_key(name)))
                    && state.standing.contains(&key)
                {
                    state.cells.insert(key);
                    return Element::retained(key);
                }
                let (content, key) = built.seat(index, place, &mut state.keys);
                let key = name.map_or(key, |name| self.named_key(name));
                let entered = match name {
                    Some(_) => rearranged,
                    None => known.contains(&at),
                };
                if fades && entered && !state.standing.contains(&key) {
                    state.arrivals.insert(key);
                }
                state.cells.insert(key);
                AnimatedPosition::default()
                    .along(self.along())
                    .anchored(index, along)
                    .arriving(state.arrivals.contains(&key))
                    .with_content(content)
                    .keyed(key)
            })
            .collect()
    }

    fn riding(
        &self,
        state: &mut CollectionState,
        built: &Cells,
        cells: Range<usize>,
    ) -> Vec<Element> {
        let riding = state.ride.as_ref().map(|_| state.generation);
        let keeps = rikka_ui::can_retain();
        let along = self.along();
        let corner = along.coord(self.corner());
        let CollectionState {
            names,
            riders,
            keys,
            spans,
            window,
            ..
        } = state;
        let start = spans.top(window.start);
        cells
            .zip(names.iter().copied())
            .map(|(index, name)| {
                let key = self.named_key(name);
                let home = corner + spans.top(index) - start;
                let place = hash_of((name, riding, riding.map(|_| home.to_bits())));
                if keeps && built.retained_key(index, place, keys).is_some() {
                    return Element::retained(key);
                }
                let (content, _) = built.seat(index, place, keys);
                Rider::new(riders, riding, name, along, home)
                    .with_content(content)
                    .keyed(key)
            })
            .collect()
    }

    fn ghosts(&self, state: &CollectionState, built: &Cells) -> Vec<Element> {
        let Some(ride) = &state.ride else {
            return Vec::new();
        };
        let along = self.along();
        let start = state.spans.top(state.window.start);
        let corner = self.corner();
        ride.ghosts
            .iter()
            .filter_map(|ghost| {
                let (top, _) = ride.seat(ghost, &state.spans)?;
                let content = ghost.cells.as_ref().unwrap_or(built).content(ghost.index);
                let home = along.coord(corner) + top - start;
                let riding = Some(state.generation);
                let rider = Rider::new(&state.riders, riding, ghost.id, along, home)
                    .placed(along.across().coord(corner), state.breadth);
                Some(rider.with_content(content).keyed(ghost.id))
            })
            .collect()
    }

    fn light(&self, state: &mut CollectionState, ctx: &mut PaintCtx<'_>, body: Rect, offset: f32) {
        let along = self.along();
        let chosen = self.selection();
        let across = state.across.max(1);
        let CollectionState {
            spans,
            fills,
            hovered,
            ride,
            riders,
            window,
            ..
        } = state;
        let surface = Surface {
            body,
            lines: self.lines,
            offset,
            across,
            breadth: self.lines.breadth(body.width(), across, self.props.gap),
            gap: self.props.gap,
            count: self.count,
            spans,
        };
        let (Some(ride), Some(named)) = (ride.as_ref(), &self.props.named) else {
            let hovered = hovered.filter(|index| *index < self.count);
            fills.paint(ctx, along, |cell| surface.cell(cell), chosen, hovered);
            return;
        };
        fills.clear(ctx);
        let mut pills: Vec<(Rect, DriftState)> = Vec::new();
        if let Some(index) = chosen.filter(|index| window.contains(index))
            && let Some(shown) = riders.shown(named(index))
        {
            pills.push((surface.cell(index), shown));
        }
        for ghost in &ride.ghosts {
            let chooses = match &ghost.cells {
                Some(cells) => cells.chooses(ghost.index),
                None => chosen == Some(ghost.index),
            };
            let (Some((top, extent)), Some(shown)) =
                (ride.seat(ghost, spans), riders.shown(ghost.id))
            else {
                continue;
            };
            if chooses {
                let extent = (extent - self.props.gap).max(0.0);
                let rect = along.rect(body.origin, top - offset, extent, 0.0, surface.breadth);
                pills.push((rect, shown));
            }
        }
        let bounds = ctx.bounds();
        let (near, far) = (
            along.coord(bounds.origin),
            along.coord(bounds.origin) + along.room(bounds),
        );
        for (rect, shown) in pills {
            let from = (along.coord(rect.origin) + shown.from).max(near);
            let clip = along.rect(
                bounds.origin,
                from - near,
                (far - from).max(0.0),
                0.0,
                along.across().room(bounds),
            );
            highlight::paint_selection(ctx, rect.translate(along.point(shown.drift, 0.0)), clip);
        }
    }

    fn remember_names(&self, state: &mut CollectionState, across: usize) {
        state.names.clear();
        if let Some(named) = &self.props.named {
            let window = state.window.clone();
            state.names.extend(window.map(|band| named(band * across)));
        }
    }

    fn measure(&self, state: &mut CollectionState, arranged: &Arranged<'_>) -> bool {
        let along = self.along();
        let across = state.across.max(1);
        let mut remeasured = false;
        for (slot, band) in state.window.clone().enumerate() {
            let mut extent: Option<f32> = None;
            for line in 0..across {
                let Some(rect) = arranged.child_rect(slot * across + line) else {
                    break;
                };
                extent = Some(extent.unwrap_or(0.0).max(along.room(rect)));
            }
            let Some(extent) = extent else {
                break;
            };
            remeasured |= state.spans.measured(band, extent + self.props.gap);
        }
        remeasured
    }
}

macro_rules! collection_methods {
    () => {
        pub fn selected(mut self, selected: impl Into<Option<usize>>) -> Self {
            self.props.selected = selected.into();
            self
        }

        pub fn uniform(mut self, extent: impl Into<::rikka_ui::Length>) -> Self {
            self.props.uniform = extent.into().fixed().map(|extent| extent.max(1.0));
            self
        }

        pub fn estimate(mut self, estimate: impl Into<::rikka_ui::Length>) -> Self {
            self.props.estimate = estimate.into().fixed().map(|estimate| estimate.max(1.0));
            self
        }

        pub fn gap(mut self, gap: impl Into<::rikka_ui::Space>) -> Self {
            self.props.gap = gap.into().px();
            self
        }

        pub fn padding(mut self, padding: impl Into<::rikka_ui::Inset>) -> Self {
            self.props.padding = padding.into();
            self
        }

        pub fn follow(mut self, follow: bool) -> Self {
            self.props.follow = follow;
            self
        }

        pub fn shows(mut self, subject: impl ::std::hash::Hash) -> Self {
            self.props.subject = ::rikka_ui::hash_of(subject);
            self
        }

        pub fn focus(mut self, focus: impl Into<Option<usize>>) -> Self {
            self.props.focus = focus.into();
            self.props.focusable = true;
            self
        }

        pub fn reveals(mut self, reveal: impl Into<Option<usize>>) -> Self {
            self.props.reveal = reveal.into();
            self
        }

        pub fn place(mut self, place: &::rikka_ui::Place) -> Self {
            self.props.place = Some(place.clone());
            self
        }

        pub fn named(mut self, named: impl Fn(usize) -> u64 + 'static) -> Self {
            self.props.named = Some(::std::rc::Rc::new(named));
            self
        }

        pub fn memo(
            mut self,
            version: impl Fn(usize) -> Option<::rikka_ui::Version> + 'static,
        ) -> Self {
            self.props.memo = Some(::std::rc::Rc::new(version));
            self
        }

        pub fn reachable(mut self, reachable: impl Fn(usize) -> bool + 'static) -> Self {
            self.props.reachable = Some(::std::rc::Rc::new(reachable));
            self
        }

        pub fn on_select<Marker>(
            mut self,
            on_select: impl ::rikka_ui::IntoHandler<(usize,), (), Marker>,
        ) -> Self {
            self.props.on_select = ::rikka_ui::Action::new(on_select);
            self.props.focusable = true;
            self
        }

        pub fn on_activate<Marker>(
            mut self,
            on_activate: impl ::rikka_ui::IntoHandler<(usize,), (), Marker>,
        ) -> Self {
            self.props.on_activate = ::rikka_ui::Action::new(on_activate);
            self
        }

        pub fn activates_on_press(mut self, activates: bool) -> Self {
            self.props.activates_on_press = activates;
            self
        }

        pub fn on_focus<Marker>(
            mut self,
            on_focus: impl ::rikka_ui::IntoHandler<(usize, ::rikka_ui::Modifiers), (), Marker>,
        ) -> Self {
            self.props.on_focus = ::rikka_ui::Action::new(on_focus);
            self.props.focusable = true;
            self
        }

        pub fn on_key<Marker>(
            mut self,
            on_key: impl ::rikka_ui::IntoHandler<
                (::rikka_ui::Key, ::rikka_ui::Modifiers, Option<usize>),
                bool,
                Marker,
            >,
        ) -> Self {
            self.props.on_key = ::rikka_ui::Action::new(on_key);
            self.props.focusable = true;
            self
        }

        pub fn on_press<Marker>(
            mut self,
            press: impl ::rikka_ui::IntoHandler<
                (::rikka_ui::PointerButton, ::rikka_ui::Modifiers),
                (),
                Marker,
            >,
        ) -> Self {
            self.props.press = ::rikka_ui::Action::new(press);
            self
        }

        pub fn on_pointer(
            mut self,
            on_pointer: impl Fn(
                &::rikka_ui::Event,
                &mut ::rikka_ui::EventCtx<'_>,
                &$crate::Surface<'_>,
            ) -> bool
            + 'static,
        ) -> Self {
            self.props.on_pointer = Some(::std::rc::Rc::new(on_pointer));
            self
        }

        pub fn on_visible<Marker>(
            mut self,
            on_visible: impl ::rikka_ui::IntoHandler<(::std::ops::Range<usize>,), (), Marker>,
        ) -> Self {
            self.props.on_visible = ::rikka_ui::Action::new(on_visible);
            self
        }

        pub fn focusable(mut self, focusable: bool) -> Self {
            self.props.focusable = focusable;
            self
        }

        pub fn autofocus(mut self, autofocus: bool) -> Self {
            self.props.autofocus = autofocus;
            self.props.focusable = true;
            self
        }

        pub fn takes_tab(mut self, takes_tab: bool) -> Self {
            self.props.takes_tab = takes_tab;
            self
        }

        pub fn lit(mut self, lit: bool) -> Self {
            self.props.lit = lit;
            self
        }

        pub fn rearrange(mut self, rearrange: $crate::Rearrange) -> Self {
            self.props.rearrange = rearrange;
            self
        }
    };
}

pub(crate) use collection_methods;
