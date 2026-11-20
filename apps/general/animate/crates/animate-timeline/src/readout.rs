use animate_cast::frame_of;
use animate_transport::Transport;
use rikka_app::prelude::*;

use crate::wording;

const FRAME: f32 = 15.0;
const FRAME_LINE: f32 = 20.0;
const DETAIL: f32 = 11.5;
const DETAIL_LINE: f32 = 16.0;
const GAP: f32 = 8.0;
pub const WIDTH: f32 = 200.0;

#[derive(Debug)]
pub struct Readout {
    transport: Transport,
}

impl Readout {
    pub fn new(transport: &Transport) -> Self {
        Self {
            transport: transport.clone(),
        }
    }
}

impl Widget for Readout {
    type State = ();

    fn paint(&self, _state: &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = palette();
        let time = self.transport.time();
        if self.transport.is_playing() {
            ctx.request_frame();
        }

        let frame = TextRun::new(format!("{:05}", frame_of(time)), FRAME)
            .medium()
            .no_wrap();
        let frame_width = measure_text(&frame, None).width;
        ctx.draw_text(
            Rect::new(
                bounds.x(),
                bounds.center().y - FRAME_LINE / 2.0,
                frame_width + 2.0,
                FRAME_LINE,
            ),
            frame,
            palette.text,
        );

        let detail = format!(
            "/ {:05}  {}",
            self.transport.last_frame(),
            wording::clock(time)
        );
        ctx.draw_text(
            Rect::new(
                bounds.x() + frame_width + GAP,
                bounds.center().y - DETAIL_LINE / 2.0 + 1.0,
                (bounds.width() - frame_width - GAP).max(0.0),
                DETAIL_LINE,
            ),
            TextRun::new(detail, DETAIL).no_wrap(),
            palette.text_muted,
        );
    }
    fn same(&self, other: &Self) -> bool {
        self.transport == other.transport
    }

    fn debug_name(&self) -> &'static str {
        "Readout"
    }
}
