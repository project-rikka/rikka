use std::sync::Arc;
use std::time::{Duration, Instant};

use cut_project::Point;
use rikka_app::prelude::*;

use super::Cut;
use crate::transport::Direction;

const TICKING: &str = "cut.ticking";
const TICK: Duration = Duration::from_millis(8);
const ZOOM_AROUND: f32 = 0.5;

impl Cut {
    pub(crate) fn play_pause(&mut self) {
        self.end_transition_preview();
        match self.transport.is_playing() {
            true => self.pause(),
            false => self.play(Direction::Forward),
        }
    }

    fn play(&mut self, direction: Direction) {
        let end = self.project().end();
        if end == 0 {
            return;
        }
        let frame = self.transport.frame();
        if direction == Direction::Forward && frame >= end {
            self.transport.seek(0);
        }
        if direction == Direction::Backward && frame <= 0 {
            return;
        }
        self.origin = self.transport.frame();
        self.transport.shuttle(direction);
        self.keep_ticking();
    }

    pub(crate) fn shuttle(&mut self, direction: Direction) {
        self.end_transition_preview();
        if self.transport.is_playing() {
            self.transport.shuttle(direction);
            return;
        }
        self.play(direction);
    }

    pub(crate) fn pause(&mut self) {
        self.transport.stop();
        self.playback.halt();
    }

    pub(crate) fn stop(&mut self) {
        if !self.transport.is_playing() {
            skip_view();
            return;
        }
        let origin = self.origin;
        self.go_to(origin);
    }

    pub(crate) fn step(&mut self, frames: i64) {
        let frame = self.transport.frame();
        self.go_to(frame + frames);
    }

    pub(crate) fn go_to(&mut self, frame: i64) {
        skip_view();
        self.place_playhead(frame);
    }

    pub(crate) fn place_playhead(&mut self, frame: i64) {
        self.end_transition_preview();
        if !self.transport.is_playing() && self.transport.frame() == frame.max(0) {
            return;
        }
        self.transport.stop();
        self.playback.halt();
        self.transport.seek(frame.max(0));
        self.scale.follow(frame.max(0) as f64);
    }

    pub(crate) fn go_to_edit(&mut self, later: bool) {
        let frame = self.transport.frame();
        let points = self.project().edit_frames();
        let edit_point = match later {
            true => points.into_iter().find(|point| *point > frame),
            false => points.into_iter().rev().find(|point| *point < frame),
        };
        if let Some(edit_point) = edit_point {
            self.go_to(edit_point);
        }
    }

    pub(crate) fn go_to_mark(&mut self, point: Point) {
        let marks = self.project().marks();
        let target = match point {
            Point::In => marks.mark_in,
            Point::Out => marks.mark_out,
        };
        if let Some(target) = target {
            self.go_to(target);
        }
    }

    pub(super) fn keep_ticking(&mut self) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        every_keyed(TICKING, TICK, |cut: &mut Cut| {
            cut.tick(Instant::now());
            skip_view();
        });
    }

    fn stop_ticking(&mut self) {
        self.ticking = false;
        cancel(TICKING);
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        let project = Arc::clone(self.project());
        let Some(speed) = self.transport.speed() else {
            self.stop_ticking();
            return;
        };
        if !self.transport.is_running() {
            if !self.playback.is_ready() && !self.transport.has_waited(now) {
                return;
            }
            self.transport.run(now);
        }
        let end = project.end();
        let frame = self.transport.exact(now);
        if let Some(start) = self
            .transition_preview()
            .and_then(|transition_preview| transition_preview.wraps(frame))
        {
            self.transport.seek(start);
            return;
        }
        if (speed > 0.0 && frame >= end as f64) || (speed < 0.0 && frame <= 0.0) {
            let rest = match speed > 0.0 {
                true => end,
                false => 0,
            };
            self.transport.halt_at(rest);
            self.playback.halt();
            self.stop_ticking();
            return;
        }
        match speed == 1.0 && self.transition_preview().is_none() {
            true => self.playback.listen(&project, &self.live, frame, now),
            false => self.playback.halt(),
        }
        self.scale.follow(frame);
    }

    fn zoom_anchor(&self) -> f32 {
        let width = self.scale.width() as f32;
        let x = self.scale.x_of(self.transport.frame() as f64);
        match x >= 0.0 && x <= width {
            true => x,
            false => width * ZOOM_AROUND,
        }
    }

    pub(crate) fn zoom(&mut self, closer: bool) {
        self.scale.zoom(closer, self.zoom_anchor());
        skip_view();
    }

    pub(crate) fn zoom_to(&mut self, level: f32) {
        self.scale.set_level(level, self.zoom_anchor());
        skip_view();
    }

    pub(crate) fn zoom_to_fit(&mut self) {
        self.scale.fit(self.project().end());
        skip_view();
    }
}
