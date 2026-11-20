use rikka_session::gestures::Axis;
use rikka_ui::{Curve, DurationToken, Motion, PaintCtx, Property, Spring, is_exiting};
use shell_wallpaper::stage::lerp;

use super::{Aim, Deck, DeckState};
use crate::stage::EXIT_DURATION;
use crate::swipe::{OPENING, PASSING, SwipeState};

const SPREAD: &str = "spread";

const STRIP: &str = "strip";

const ARRIVING: Curve = Motion::enter(DurationToken::Medium2).curve(Property::Spatial);

const DEPARTING: Curve = Motion::exit(EXIT_DURATION).curve(Property::Spatial);

pub(super) const SETTLE: Curve = Curve::Spring(Spring::new(1.0, 420.0));

const OVERRUN: f32 = 0.25;

impl Deck {
    pub(super) fn init_scroll(&self, state: &mut DeckState, swiped: &SwipeState) {
        if std::mem::replace(&mut state.born, true) {
            return;
        }
        if let SwipeState::Released(track) = swiped {
            state.thrown = Some(track.id);
            state.passed = Some(track.id);
        }
    }

    pub(super) fn spread(
        &self,
        state: &mut DeckState,
        swiped: &SwipeState,
        ctx: &mut PaintCtx<'_>,
    ) -> f32 {
        match swiped {
            SwipeState::Tracking(track) if track.axis == Axis::Vertical => {
                let spread = track.spread();
                ctx.jump(SPREAD, spread);
                state.flung = false;
                return spread;
            }
            SwipeState::Released(track)
                if track.axis == Axis::Vertical && state.thrown != Some(track.id) =>
            {
                state.thrown = Some(track.id);
                match track.stays_open() {
                    true => {
                        ctx.fling(SPREAD, track.spread(), track.speed / OPENING);
                        state.flung = true;
                    }
                    false => {
                        ctx.jump(SPREAD, track.spread());
                        state.closing = Some(track.id);
                    }
                }
            }
            _ => {}
        }
        if let SwipeState::Released(track) = swiped
            && state.closing == Some(track.id)
            && !is_exiting()
        {
            return track.spread();
        }
        let entering = match state.flung {
            true => SETTLE,
            false => ARRIVING,
        };
        ctx.progress(SPREAD, !is_exiting(), entering, DEPARTING)
            .clamp(0.0, 1.0)
    }

    pub(super) fn focus(
        &self,
        state: &mut DeckState,
        swiped: &SwipeState,
        spread: f32,
        pitch: f32,
        ctx: &mut PaintCtx<'_>,
    ) -> f32 {
        let board = &self.board;
        self.rebase(state, ctx);
        let last = board.spaces.len().saturating_sub(1) as f32;
        let clamp_to_track = |at: f32| at.clamp(-OVERRUN, last + OVERRUN);

        match swiped {
            SwipeState::Tracking(track) if track.sideways_on(&board.output) => {
                let start = started(state, track.id);
                let at = clamp_to_track(start + track.passed());
                ctx.jump(STRIP, at);
                state.focus = at;
                return at;
            }
            SwipeState::Released(track)
                if track.sideways_on(&board.output) && state.passed != Some(track.id) =>
            {
                state.passed = Some(track.id);
                let start = started(state, track.id);
                let at = clamp_to_track(start + track.passed());
                let aimed = (board.showing as i32 + track.step()).clamp(0, last as i32) as usize;
                state.aim = Some(Aim {
                    at: aimed,
                    showing: board.showing,
                });
                ctx.fling(STRIP, at, track.speed / PASSING);
            }
            _ => {}
        }
        if let Some(scrolling) = &state.scrolling {
            let at = clamp_to_track(scrolling.start + scrolling.travel / pitch.max(1.0));
            ctx.jump(STRIP, at);
            state.focus = at;
            return at;
        }
        let target = state
            .aim
            .filter(|aim| aim.showing == board.showing)
            .map_or(board.showing, |aim| aim.at) as f32;
        let at = ctx.animate_curve(STRIP, target, SETTLE);
        state.focus = at;
        lerp(target, at, spread)
    }

    pub(super) fn aim(&self, state: &mut DeckState, step: i32) {
        let last = self.board.spaces.len().saturating_sub(1) as i32;
        let at = (self.board.showing as i32 + step).clamp(0, last) as usize;
        state.aim = Some(Aim {
            at,
            showing: self.board.showing,
        });
    }

    fn rebase(&self, state: &mut DeckState, ctx: &mut PaintCtx<'_>) {
        let numbers = self.board.spaces.iter().map(|space| space.number);
        if state.listed.iter().copied().eq(numbers.clone()) {
            return;
        }
        let old = std::mem::replace(&mut state.listed, numbers.collect());
        if old.is_empty() {
            return;
        }
        let near = state.focus.round().clamp(0.0, (old.len() - 1) as f32);
        let Some(number) = old.get(near as usize) else {
            return;
        };
        let Some(now) = state
            .listed
            .iter()
            .position(|clamp_to_track| clamp_to_track == number)
        else {
            return;
        };
        let by = now as f32 - near;
        if by == 0.0 {
            return;
        }
        ctx.shift(STRIP, by);
        state.focus += by;
        if let Some((_, start)) = &mut state.followed {
            *start += by;
        }
        if let Some(scrolling) = &mut state.scrolling {
            scrolling.start += by;
        }
        state.aim = None;
    }
}

fn started(state: &mut DeckState, id: u64) -> f32 {
    match state.followed {
        Some((followed, start)) if followed == id => start,
        _ => {
            state.followed = Some((id, state.focus));
            state.focus
        }
    }
}
