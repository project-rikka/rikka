use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_input::PointerRestrictions;
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_seat::{SeatId, VirtualPointerInput};

use super::pending::{AbsoluteSource, Moved, PendingEvent};
use super::translate::scaled;
use crate::outputs::Outputs;
use crate::state::Compositor;

impl Compositor {
    pub(super) fn virtual_pointer(
        &mut self,
        seat: SeatId,
        output: Option<&WlOutput>,
        input: VirtualPointerInput,
    ) {
        if seat == self.input.agent.seat() {
            return self.agent_pointer(output, input);
        }
        match input {
            VirtualPointerInput::Motion { delta, time } => {
                let to = self
                    .outputs
                    .route(false, PointerRestrictions::empty())
                    .clamp(self.cursor.position() + delta);
                self.input.pending.push(PendingEvent::Motion {
                    to,
                    moved: Some(Moved {
                        delta,
                        unaccelerated: delta,
                    }),
                    placed: false,
                    time,
                });
            }
            VirtualPointerInput::MotionAbsolute { fraction, time } => {
                let pointed = self.cursor.position();
                let Some(area) = self.outputs.virtual_area(output, pointed) else {
                    return;
                };
                self.input.pending.push_pointed(
                    AbsoluteSource::Virtual,
                    stretch(area, fraction),
                    time,
                );
            }
            VirtualPointerInput::Button(button) => self.input.pending.push(PendingEvent::Button {
                code: button.code,
                press: button.press,
                time: button.time,
            }),
            VirtualPointerInput::Axis(axis) => {
                let axis = scaled(axis, self.input.rules());
                self.input.pending.push(PendingEvent::Axis(axis));
            }
        }
        self.frames.input_arrived();
    }
}

pub(crate) fn stretch(
    area: Rect<i32, Logical>,
    fraction: Point<f64, Logical>,
) -> Point<f64, Logical> {
    let area = area.to_f64();
    Point::new(
        area.loc.x + fraction.x * area.size.w,
        area.loc.y + fraction.y * area.size.h,
    )
}

impl Outputs {
    pub(crate) fn virtual_area(
        &self,
        output: Option<&WlOutput>,
        pointed: Point<f64, Logical>,
    ) -> Option<Rect<i32, Logical>> {
        let named = output
            .and_then(|output| self.wayland().output_of(output))
            .and_then(|output| self.get(output));
        named
            .or_else(|| self.containing(pointed))
            .or_else(|| self.enabled().next())?
            .area()
    }
}
