use animate_cast::frame_of;
use animate_transport::Transport;
use rikka_app::prelude::*;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_timeline::{Cue, Head, Playhead, TimeLane};

use crate::layout::RULER_HEIGHT;
use crate::state::Timeline;

#[derive(Debug, PartialEq)]
pub struct Position {
    timeline: Timeline,
    transport: Transport,
}

impl Cue for Position {
    fn is_playing(&self) -> bool {
        self.transport.is_playing()
    }

    fn frame(&self) -> f64 {
        let time = self.transport.time();
        match self.transport.is_playing() {
            true => f64::from(time * FRAMES_PER_SECOND),
            false => f64::from(frame_of(time)),
        }
    }

    fn lane(&self, bounds: Rect) -> TimeLane {
        let geometry = self.timeline.geometry(bounds);
        let ruler = geometry.ruler();
        let reach = geometry.keys().max_y() - bounds.y();
        TimeLane {
            area: Rect::new(ruler.x(), bounds.y(), ruler.width(), reach),
            ..geometry.lane()
        }
    }
}

pub fn playhead(timeline: &Timeline, transport: &Transport) -> Playhead<Position> {
    Playhead::new(
        Position {
            timeline: timeline.clone(),
            transport: transport.clone(),
        },
        Head::Numbered,
        RULER_HEIGHT,
    )
}
