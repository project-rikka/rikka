use std::time::Instant;

use rikka_app::prelude::*;
use rikka_timeline::{Cue, Head, Playhead, TimeLane};

use crate::timeline::ruler;
use crate::timeline::scale::Scale;
use crate::transport::Transport;

#[derive(Debug, PartialEq)]
pub(crate) struct Position {
    scale: Scale,
    transport: Transport,
    left: f32,
}

impl Cue for Position {
    fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }

    fn frame(&self) -> f64 {
        let exact = self.transport.exact(Instant::now());
        match self.transport.is_running() {
            true => exact,
            false => exact.floor(),
        }
    }

    fn lane(&self, bounds: Rect) -> TimeLane {
        let area = Rect::new(
            bounds.x() + self.left,
            bounds.y(),
            (bounds.width() - self.left).max(0.0),
            bounds.height(),
        );
        TimeLane {
            area,
            origin: area.x(),
            scale: self.scale.shown(),
        }
    }
}

pub(crate) fn playhead(scale: &Scale, transport: &Transport, left: f32) -> Playhead<Position> {
    Playhead::new(
        Position {
            scale: scale.clone(),
            transport: transport.clone(),
            left,
        },
        Head::Tab,
        ruler::HEIGHT,
    )
}
