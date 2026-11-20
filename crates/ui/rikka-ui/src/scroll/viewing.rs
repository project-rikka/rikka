use rikka_types::Rect;

use crate::input::{Event, EventCtx};
use crate::paint::PaintCtx;
use crate::scroll::bar::{self, Grabbed, Grip, Scroll};
use crate::scroll::place::{Place, ScrollPosition};
use crate::scroll::reveal::Reveal;
use crate::style::Axis;

#[derive(Debug, Default)]
pub struct Viewing {
    pub scroll: Scroll,
    pub grip: Grip,
    adrift: bool,
    homing: bool,
    swallows_reveal: bool,
}

impl Viewing {
    pub fn adopt(&mut self, place: Option<&Place>, subject: Option<u64>) -> Option<ScrollPosition> {
        let stand = self.scroll.adopt(place, subject)?;
        self.adrift = !stand.ended;
        self.homing = false;
        Some(stand)
    }

    pub fn opened(&mut self) {
        self.scroll.snap(0.0);
        self.adrift = false;
        self.homing = false;
    }

    pub fn is_adrift(&self) -> bool {
        self.adrift
    }

    pub fn holds_end(&self) -> bool {
        !self.adrift && !self.homing
    }

    pub fn settle(&mut self, furthest: f32) {
        if !bar::at_end(self.scroll.target(), furthest) {
            self.leave();
        }
    }

    pub fn heed(&mut self, place: Option<&Place>, furthest: f32, room: f32) {
        if self.scroll.heed(place, furthest, room) {
            self.adrift = false;
            self.homing = true;
        }
    }

    pub fn follow(&mut self, furthest: f32, flying: bool) {
        if furthest <= 0.0 {
            self.adrift = false;
        }
        if self.adrift {
            return;
        }
        if self.homing && bar::at_end(self.scroll.rendered(), furthest) {
            self.homing = false;
        }
        match flying || self.homing {
            true => self.scroll.aim(furthest),
            false => self.scroll.snap(furthest),
        }
    }

    pub fn swallows_next_reveal(&mut self, swallows: bool) {
        self.swallows_reveal = swallows;
    }

    pub fn reveal(
        &mut self,
        axis: Axis,
        room: Rect,
        rect: Rect,
        furthest: f32,
        reveal: Reveal,
    ) -> bool {
        if furthest <= 0.0 {
            return false;
        }
        let start = axis.coord(rect.origin) - axis.coord(room.origin) + self.scroll.rendered();
        match reveal {
            Reveal::Nearest if self.swallows_reveal => {}
            Reveal::Nearest => self.aim_at(start, axis.room(rect), axis.room(room), furthest),
            Reveal::Start => self.steer_to(start.clamp(0.0, furthest), furthest),
        }
        true
    }

    pub fn aim_at(&mut self, start: f32, height: f32, room: f32, furthest: f32) {
        let from = self.scroll.target().clamp(0.0, furthest);
        self.steer_to(bar::follow(from, start, height, room, furthest), furthest);
    }

    fn steer_to(&mut self, target: f32, furthest: f32) {
        let from = self.scroll.target().clamp(0.0, furthest);
        self.scroll.aim(target);
        self.steered(from, furthest);
    }

    pub fn grabbed(
        &mut self,
        axis: Axis,
        track: Rect,
        content: f32,
        furthest: f32,
        event: &Event,
        ctx: &mut EventCtx<'_>,
    ) -> bool {
        let took = self
            .grip
            .handle_event(axis, track, content, self.scroll.rendered(), event, ctx);
        match took {
            Grabbed::Missed => false,
            Grabbed::Moved(to) => {
                let from = self.scroll.target().clamp(0.0, furthest);
                self.scroll.snap(to);
                self.steered(from, furthest);
                true
            }
            Grabbed::Rested => true,
        }
    }

    pub fn nudged(&mut self, delta: f32, furthest: f32) -> bool {
        let from = self.scroll.target().clamp(0.0, furthest);
        if self.scroll.nudge(delta, furthest) {
            self.steered(from, furthest);
            return true;
        }
        if delta > 0.0 && self.adrift && bar::at_end(from, furthest) {
            self.rejoin(furthest);
        }
        false
    }

    fn steered(&mut self, from: f32, furthest: f32) {
        let to = self.scroll.target();
        if to < from {
            self.leave();
        } else if to > from && bar::at_end(to, furthest) {
            self.rejoin(furthest);
        }
    }

    fn leave(&mut self) {
        self.adrift = true;
        self.homing = false;
    }

    fn rejoin(&mut self, furthest: f32) {
        self.adrift = false;
        self.homing = !bar::at_end(self.scroll.rendered(), furthest);
    }

    pub fn paint_bar(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        axis: Axis,
        track: Rect,
        content: f32,
        offset: f32,
    ) {
        let dragging = self.grip.is_dragging();
        let hovered = ctx.hovered();
        bar::paint_thumb(ctx, axis, track, content, offset, (hovered, dragging));
    }
}
