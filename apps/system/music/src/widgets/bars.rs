use std::time::Instant;

use rikka_app::prelude::*;
use rikka_ui::motion;

const BARS: usize = 3;
const GAP: f32 = 2.0;
const PHASES: [f32; BARS] = [0.0, 1.7, 3.1];
const SPEEDS: [f32; BARS] = [7.3, 9.1, 6.2];
const REST: f32 = 0.3;

pub(crate) struct Bars {
    playing: bool,
    side: f32,
}

pub(crate) fn bars(playing: bool, side: f32) -> Bars {
    Bars { playing, side }
}

#[derive(Debug)]
pub(crate) struct BarsState {
    born: Instant,
}

impl Default for BarsState {
    fn default() -> Self {
        BarsState {
            born: Instant::now(),
        }
    }
}

impl Widget for Bars {
    type State = BarsState;

    fn layout(&self) -> Layout {
        Layout::DEFAULT
            .size(Length::px(self.side), Length::px(self.side))
            .fixed()
    }

    fn paint(&self, state: &mut BarsState, ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let color = ctx.palette().accent;
        let seconds = ctx.now().duration_since(state.born).as_secs_f32();
        let width = (bounds.width() - GAP * (BARS - 1) as f32) / BARS as f32;
        let swing = ctx.animate_curve("swing", if self.playing { 1.0 } else { 0.0 }, motion::FILL);
        for index in 0..BARS {
            let wave = ((seconds * SPEEDS[index] + PHASES[index]).sin() + 1.0) / 2.0;
            let height = bounds.height() * (REST + (1.0 - REST) * wave * swing);
            let x = bounds.x() + index as f32 * (width + GAP);
            ctx.draw_quad(
                Quad::new(Rect::new(x, bounds.max_y() - height, width, height))
                    .fill(color)
                    .radius(width / 2.0),
            );
        }
        if self.playing {
            ctx.request_frame();
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.playing == other.playing && self.side == other.side
    }

    fn debug_name(&self) -> &'static str {
        "Bars"
    }
}

impl View for Bars {
    fn build(self) -> Element {
        Element::new(self)
    }
}

impl std::fmt::Debug for Bars {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Bars")
            .field("playing", &self.playing)
            .finish_non_exhaustive()
    }
}
