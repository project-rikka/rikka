use rikka_text::Affinity;
use rikka_types::{Point, ScrollDelta};
use rikka_ui::scroll::bar::{self, Grabbed};
use rikka_ui::text::{Endpoints, Granularity, SelectionOrigin};
use rikka_ui::{Axis, Event, EventCtx};

use crate::text::editing::Editing;
use crate::text::editing::carry::DragOrigin;
use crate::text::editing::gesture::Gesture;
use crate::text::editing::state::EditingState;

const SIDEWAYS_STEP: f32 = 0.1;

impl Editing {
    pub(crate) fn press(
        &self,
        state: &mut EditingState,
        at: Point,
        extend: bool,
        ctx: &mut EventCtx<'_>,
    ) {
        let Some((offset, affinity)) = self.cursor_at(state, at) else {
            return;
        };
        let presses = ctx.presses();
        if presses == 1 && !extend && self.holds_selection_at(offset) {
            state.drag_origin = Some(DragOrigin { at, offset });
            state.goal = None;
            self.document.interrupt();
            ctx.capture_pointer();
            return;
        }
        let changed = match presses == 1 && extend {
            true => self.extend_selection(state, offset, affinity),
            false => self.start_selection(state, offset, affinity, self.granularity(presses)),
        };
        state.dragging = true;
        state.goal = None;
        state.follow = true;
        self.document.interrupt();
        ctx.capture_pointer();
        if changed {
            self.changed(ctx);
        }
    }

    pub(crate) fn drag(&self, state: &mut EditingState, to: Point, ctx: &mut EventCtx<'_>) {
        let Some((offset, affinity)) = self.cursor_at(state, to) else {
            return;
        };
        let frame = state.frame;
        let beyond = (to.y - frame.room.max_y()).max(frame.room.y() - to.y);
        if beyond > 0.0 && self.options.bars {
            let furthest = bar::max_scroll(Axis::Vertical, frame.room, frame.content_height);
            let travel = match to.y > frame.room.center().y {
                true => frame.line_height,
                false => -frame.line_height,
            };
            state.down.nudge(travel, furthest);
        }
        state.follow = true;
        if self.extend_selection(state, offset, affinity) || beyond > 0.0 {
            self.changed(ctx);
        }
    }

    fn granularity(&self, presses: u32) -> Granularity {
        match Granularity::from_click_count(presses) {
            Granularity::Word if self.options.secret => Granularity::Line,
            granularity => granularity,
        }
    }

    fn start_selection(
        &self,
        state: &mut EditingState,
        offset: usize,
        affinity: Affinity,
        granularity: Granularity,
    ) -> bool {
        if granularity == Granularity::Character {
            state.gesture = None;
            return self.document.place_cursor(offset, affinity, false);
        }
        let rule = self.word_rule();
        let single_line = self.options.single_line;
        self.document.edit(|content| {
            let unit = match (granularity, single_line) {
                (Granularity::Line, true) => 0..content.len(),
                _ => content.unit_at(offset, affinity, granularity, rule),
            };
            let changed = content.select(unit.start, unit.end);
            state.gesture = Some(Gesture::record(
                SelectionOrigin::new(granularity, unit),
                content,
            ));
            changed
        })
    }

    fn extend_selection(
        &self,
        state: &mut EditingState,
        offset: usize,
        affinity: Affinity,
    ) -> bool {
        let rule = self.word_rule();
        self.document.edit(|content| {
            let Some(gesture) = state.gesture.as_mut().filter(|held| held.holds(content)) else {
                return content.place_cursor(offset, affinity, true);
            };
            let origin = gesture.origin();
            let unit = content.unit_at(offset, affinity, origin.granularity(), rule);
            let Endpoints { anchor, focus } = origin.extend(unit);
            let changed = content.select(anchor, focus);
            *gesture = Gesture::record(origin, content);
            changed
        })
    }

    pub(super) fn nudge_towards(&self, state: &mut EditingState, to: Point) {
        if !self.options.bars {
            return;
        }
        let frame = state.frame;
        let near = frame.line_height;
        let travel = if to.y > frame.room.max_y() - near {
            frame.line_height
        } else if to.y < frame.room.y() + near {
            -frame.line_height
        } else {
            return;
        };
        let furthest = bar::max_scroll(Axis::Vertical, frame.room, frame.content_height);
        state.down.nudge(travel, furthest);
    }

    pub(super) fn offset_at(&self, state: &EditingState, at: Point) -> Option<usize> {
        self.cursor_at(state, at).map(|(offset, _)| offset)
    }

    pub(super) fn cursor_at(&self, state: &EditingState, at: Point) -> Option<(usize, Affinity)> {
        let content = self.document.content();
        let shown = self.shown(&content);
        let within = state.frame.within_sheet(at);
        let (drawn, affinity) = state
            .block
            .cursor_at(&shown, &state.rows, within.x, within.y)?;
        Some((shown.document_offset(drawn), affinity))
    }

    pub(crate) fn grabbed(
        &self,
        state: &mut EditingState,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        if !self.options.bars {
            return false;
        }
        let frame = state.frame;
        for axis in [Axis::Vertical, Axis::Horizontal] {
            let (content, grip, scroll) = match axis {
                Axis::Vertical => (frame.content_height, &mut state.down_grip, &mut state.down),
                Axis::Horizontal => (
                    frame.content_width,
                    &mut state.across_grip,
                    &mut state.across,
                ),
            };
            let track = bar::track(axis, frame.bounds, frame.room);
            match grip.handle_event(axis, track, content, scroll.rendered(), event, ctx) {
                Grabbed::Missed => continue,
                Grabbed::Moved(to) => scroll.snap(to),
                Grabbed::Rested => {}
            }
            return true;
        }
        false
    }

    pub(crate) fn holds_bar(&self, state: &EditingState) -> bool {
        state.down_grip.is_dragging() || state.across_grip.is_dragging()
    }

    pub(crate) fn scrolled(
        &self,
        state: &mut EditingState,
        delta: ScrollDelta,
        sideways: bool,
    ) -> bool {
        let frame = state.frame;
        let offset = delta.offset();
        let (across, down) = match sideways {
            true => (offset.y + offset.x, 0.0),
            false => (offset.x, offset.y),
        };
        let across_furthest = bar::max_scroll(Axis::Horizontal, frame.room, frame.content_width);
        let down_furthest = bar::max_scroll(Axis::Vertical, frame.room, frame.content_height);
        let upright = down.abs() >= across.abs();
        let across = across * frame.room.width() * SIDEWAYS_STEP / frame.line_height.max(1.0);
        let led = match upright {
            true => state.down.nudge(down, down_furthest),
            false => state.across.nudge(across, across_furthest),
        };
        if !led {
            return false;
        }
        match upright {
            true => state.across.nudge(across, across_furthest),
            false => state.down.nudge(down, down_furthest),
        };
        true
    }
}
