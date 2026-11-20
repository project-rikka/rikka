use std::hash::Hash;

use rikka_types::{Point, Rect, Size};
use rikka_ui::scroll::bar::{self};
use rikka_ui::scroll::{Place, ScrollPosition, Viewing};
use rikka_ui::{
    Arranged, Axis, Behavior, Element, Event, EventCtx, Layout, Length, PaintCtx, Reveal, Style,
    Styled, View, Widget, hash_of,
};

#[must_use]
pub struct Scrollable {
    child: Option<Element>,
    follow: bool,
    subject: Option<u64>,
    place: Option<Place>,
    style: Style,
}

pub fn scrollable(child: impl View) -> Scrollable {
    Scrollable {
        child: Some(child.build()),
        follow: false,
        subject: None,
        place: None,
        style: Style::default(),
    }
}

impl Scrollable {
    pub fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    pub fn shows(mut self, subject: impl Hash) -> Self {
        self.subject = Some(hash_of(subject));
        self
    }

    pub fn place(mut self, place: &Place) -> Self {
        self.place = Some(place.clone());
        self
    }

    fn take_place(&self, state: &mut ScrollState) -> bool {
        if let Some(subject) = self.subject
            && state.subject != Some(subject)
        {
            state.subject = Some(subject);
            state.viewing.opened();
            return true;
        }
        state
            .viewing
            .adopt(self.place.as_ref(), self.subject)
            .is_some()
    }
}

#[derive(Debug, Default)]
#[doc(hidden)]
pub struct ScrollState {
    viewing: Viewing,
    subject: Option<u64>,
    shown: f32,
    touched: bool,
}

fn clamp_offset(offset: f32, bounds: Rect, content_size: Size) -> f32 {
    offset.clamp(
        0.0,
        bar::max_scroll(Axis::Vertical, bounds, content_size.height),
    )
}

impl Widget for Scrollable {
    type State = ScrollState;

    fn layout(&self) -> Layout {
        Layout::column()
            .scrolling(Axis::Vertical)
            .size(Length::FILL, Length::HUG)
            .with(&self.style)
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::CLIPS | Behavior::TRACKS_HOVER
    }

    fn children_offset(&self, state: &ScrollState, bounds: Rect, content_size: Size) -> Point {
        let _ = (bounds, content_size);
        Point::new(0.0, -state.shown)
    }

    fn reveal(
        &self,
        state: &mut ScrollState,
        room: Rect,
        content_size: Size,
        rect: Rect,
        reveal: Reveal,
    ) -> bool {
        let furthest = bar::max_scroll(Axis::Vertical, room, content_size.height);
        state
            .viewing
            .reveal(Axis::Vertical, room, rect, furthest, reveal)
    }

    fn arranged(&self, state: &mut ScrollState, arranged: Arranged<'_>) -> bool {
        if let Some(anchor) = arranged.anchor() {
            state.viewing.scroll.shift(anchor.moved());
            state.shown += anchor.moved();
            state.touched |= anchor.touched;
        }
        false
    }

    fn paint(&self, state: &mut ScrollState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let content = ctx.content_size();
        let furthest = bar::max_scroll(Axis::Vertical, bounds, content.height);
        let placed = self.take_place(state);
        state.viewing.swallows_next_reveal(placed);
        if std::mem::take(&mut state.touched) {
            state.viewing.settle(furthest);
        }
        state
            .viewing
            .heed(self.place.as_ref(), furthest, bounds.height());
        if self.follow {
            state.viewing.follow(furthest, true);
        }
        let offset = state.viewing.scroll.advance(ctx, Axis::Vertical, furthest);
        state.shown = ctx.snap(clamp_offset(offset, bounds, content));
        let stand = ScrollPosition {
            subject: self.subject,
            ..state.viewing.scroll.stand(furthest, bounds.height())
        };
        state.viewing.scroll.publish(self.place.as_ref(), stand);
        state
            .viewing
            .paint_bar(ctx, Axis::Vertical, bounds, content.height, offset);
    }

    fn event(&self, state: &mut ScrollState, event: &Event, ctx: &mut EventCtx<'_>) {
        state.viewing.adopt(self.place.as_ref(), self.subject);
        let bounds = ctx.bounds();
        let content = ctx.content_size();
        let furthest = bar::max_scroll(Axis::Vertical, bounds, content.height);

        if state
            .viewing
            .grabbed(Axis::Vertical, bounds, content.height, furthest, event, ctx)
        {
            return;
        }

        let Event::Scroll { delta, source, .. } = event else {
            return;
        };
        let travel = bar::travel(Axis::Vertical, *delta, *source);
        if state.viewing.nudged(travel, furthest) {
            ctx.request_paint();
            ctx.set_handled();
        }
    }
}

impl std::fmt::Debug for Scrollable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scrollable").finish_non_exhaustive()
    }
}

impl View for Scrollable {
    fn build(mut self) -> Element {
        let children = self.child.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for Scrollable {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
