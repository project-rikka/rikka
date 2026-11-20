use std::time::Duration;

use rikka_compositor_input::InputEvent;
use rikka_compositor_seat::{TouchDown, TouchMotion, TouchUp};

use super::pending::{PendingEvent, Spot, Touched};
use crate::context::input_ref;
use crate::context::windows_ctx;
use crate::state::Compositor;

pub(super) fn pending_event(event: &InputEvent) -> Option<PendingEvent> {
    let (touched, time) = match *event {
        InputEvent::TouchDown {
            device,
            time,
            slot,
            position,
        } => (
            Touched::Down {
                slot,
                spot: Spot::On { device, position },
            },
            time,
        ),
        InputEvent::TouchMotion {
            device,
            time,
            slot,
            position,
        } => (
            Touched::Motion {
                slot,
                spot: Spot::On { device, position },
            },
            time,
        ),
        InputEvent::TouchUp { time, slot, .. } => (Touched::Up { slot }, time),
        InputEvent::TouchCancel { time, .. } => (Touched::Cancel, time),
        InputEvent::TouchFrame { time, .. } => (Touched::Frame, time),
        _ => return None,
    };
    Some(PendingEvent::Touch {
        touched,
        time: time.as_monotonic(),
    })
}

impl Compositor {
    pub(super) fn touched(&mut self, touched: Touched, time: Duration) {
        let seat = self.input.seat;
        match touched {
            Touched::Down { slot, spot } => {
                let Some(location) = input_ref!(self).reach(spot) else {
                    return;
                };
                let first = self.seats.get(seat).is_some_and(|seat_state| {
                    !seat_state.touch().active() && !seat_state.touch().grabbed()
                });
                if first {
                    windows_ctx!(self).focus_under(location);
                    windows_ctx!(self).touch_caption(slot, location, time);
                }
                let under = input_ref!(self).focus_at(location);
                let mut touch = self.person();
                touch.activity();
                touch.touch_down(TouchDown {
                    slot,
                    location,
                    under,
                    time,
                });
            }
            Touched::Motion { slot, spot } => {
                let Some(location) = input_ref!(self).reach(spot) else {
                    return;
                };
                let under = input_ref!(self).focus_at(location);
                let mut touch = self.person();
                touch.activity();
                touch.touch_motion(TouchMotion {
                    slot,
                    location,
                    under,
                    time,
                });
            }
            Touched::Up { slot } => {
                let mut touch = self.person();
                touch.activity();
                touch.touch_up(TouchUp { slot, time });
            }
            Touched::Cancel => self.person().touch_cancel(),
            Touched::Frame => self.person().touch_frame(),
        }
    }
}
