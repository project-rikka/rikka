use std::time::Duration;

use rikka_compositor_desktop::{Hit, Target};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_input::PointerRestrictions;
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_seat::{
    Axis, Button, Focus, Key, KeyFilter, KeyboardFocus, Keystroke, Motion, VirtualPointerInput,
};
use rikka_compositor_types::Press;

use crate::context::{InputCtx, InputRef, agent_seat, input_ctx, input_ref};
use crate::context::{WindowsCtx, windows_ctx};
use crate::input::{PendingEvent, stretch};
use crate::render::monotonic;
use crate::state::Compositor;

impl Compositor {
    pub(crate) fn agent_pointer(&mut self, output: Option<&WlOutput>, input: VirtualPointerInput) {
        match input {
            VirtualPointerInput::Motion { delta, time } => {
                let at = self.input.agent.at + delta;
                self.agent_moved_to(at, time);
            }
            VirtualPointerInput::MotionAbsolute { fraction, time } => {
                let Some(area) = self.outputs.virtual_area(output, self.input.agent.at) else {
                    return;
                };
                self.agent_moved_to(stretch(area, fraction), time);
            }
            VirtualPointerInput::Button(button) => self.agent_button(button),
            VirtualPointerInput::Axis(axis) => self.agent_axis(axis),
        }
    }

    pub(crate) fn agent_keystroke(&mut self, keystroke: &Keystroke) -> KeyFilter {
        let rekeyed = &mut self.input.agent.rekeyed;
        let intercepted = match keystroke.press {
            Press::Pressed if self.input.agent.relayed => {
                rekeyed.push(keystroke.code);
                true
            }
            Press::Pressed => false,
            Press::Released => match rekeyed.iter().position(|code| *code == keystroke.code) {
                Some(index) => {
                    rekeyed.remove(index);
                    true
                }
                None => false,
            },
        };
        if !intercepted {
            return KeyFilter::Forward;
        }
        self.person().key(Key {
            code: keystroke.code,
            press: keystroke.press,
            time: keystroke.time,
            source: keystroke.source,
        });
        KeyFilter::Consume
    }

    fn agent_moved_to(&mut self, location: Point<f64, Logical>, time: Duration) {
        let location = input_ctx!(self).agent_placed_at(location);
        let under = input_ctx!(self).agent_target(location);
        let relayed = self.input.agent.relayed;
        let mut pointer = self.agent_seat();
        pointer.motion(Motion {
            location,
            under,
            time,
        });
        pointer.frame();
        if relayed {
            input_ctx!(self).queue_person_event(PendingEvent::Motion {
                to: location,
                moved: None,
                placed: false,
                time,
            });
        }
    }

    fn agent_button(&mut self, button: Button) {
        if self.input.agent.relayed {
            return input_ctx!(self).queue_person_event(PendingEvent::Button {
                code: button.code,
                press: button.press,
                time: button.time,
            });
        }
        if button.press == Press::Pressed {
            windows_ctx!(self).focus_under_agent();
        }
        let mut pointer = self.agent_seat();
        pointer.button(button);
        pointer.frame();
    }

    fn agent_axis(&mut self, axis: Axis) {
        if self.input.agent.relayed {
            return input_ctx!(self).queue_person_event(PendingEvent::Axis(axis));
        }
        let mut pointer = self.agent_seat();
        pointer.axis(axis);
        pointer.frame();
    }
}

impl InputCtx<'_> {
    fn agent_placed_at(&mut self, location: Point<f64, Logical>) -> Point<f64, Logical> {
        let location = self
            .outputs
            .route(false, PointerRestrictions::empty())
            .clamp(location);
        let was = std::mem::replace(&mut self.input.agent.at, location);
        if self.input.agent.shown() {
            self.agent_damaged(was);
            self.agent_damaged(location);
        }
        location
    }

    fn queue_person_event(&mut self, event: PendingEvent) {
        self.input.pending.push(event);
        self.frames.input_arrived();
    }

    pub(crate) fn agent_warped(&mut self, location: Point<f64, Logical>) {
        let location = self.agent_placed_at(location);
        let under = self.agent_target(location);
        agent_seat!(self).warp(Motion {
            location,
            under,
            time: monotonic(),
        });
    }

    fn agent_target(&mut self, location: Point<f64, Logical>) -> Option<Focus> {
        let relayed = input_ref!(self).relayed_under(location);
        self.input.agent.relayed = relayed;
        match relayed {
            true => None,
            false => input_ref!(self).focus_at(location),
        }
    }
}

impl InputRef<'_> {
    fn relayed_under(&self, at: Point<f64, Logical>) -> bool {
        match self.hit_under(at) {
            Some(Hit::Surface(under)) => self.windows.is_x11_target(under.target),
            Some(Hit::Frame { .. }) => true,
            None => false,
        }
    }
}

impl WindowsCtx<'_> {
    fn focus_under_agent(&mut self) {
        let at = self.input.agent.at;
        let focus = input_ref!(self).element_under(at).map(|under| {
            let surface = match self.windows.desktop.root_of(under.target) {
                Target::Window(window) => self
                    .windows
                    .desktop
                    .window(window)
                    .and_then(|record| record.surface()),
                _ => None,
            };
            KeyboardFocus::Surface(surface.unwrap_or(under.surface))
        });
        agent_seat!(self).set_keyboard_focus(focus);
    }
}
