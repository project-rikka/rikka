use std::time::Duration;

use rikka_compositor_desktop::{Target, Under, WindowId};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_protocols::zwlr_layer_surface_v1::KeyboardInteractivity;
use rikka_compositor_seat::{
    Axis, Button, Focus, Key, KeySource, KeyboardGrab, Motion, RelativeMotion,
};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::Press;

use super::pending::{Moved, PendingEvent};
use crate::context::{InputCtx, InputRef, input_ctx, input_ref, parts, person, windows_ref};
use crate::context::{WindowsCtx, switcher_ctx, windows_ctx};
use crate::state::Compositor;
use crate::windows::around;

impl Compositor {
    pub(crate) fn dispatch_pending_input(&mut self) {
        let pending = self.input.pending.take();
        if !pending.is_empty() {
            input_ctx!(self).person_active();
        }
        for event in pending {
            match event {
                PendingEvent::Key {
                    device,
                    code,
                    press,
                    time,
                } => {
                    let mut keyboard = self.person();
                    keyboard.activity();
                    keyboard.key(Key {
                        code,
                        press,
                        time,
                        source: KeySource::Device(device),
                    });
                }
                PendingEvent::Motion {
                    to,
                    moved,
                    placed,
                    time,
                } => self.pointer_moved(to, moved, placed, time),
                PendingEvent::Button { code, press, time } => {
                    self.pointer_button(code, press, time)
                }
                PendingEvent::Axis(axis) => self.pointer_axis(axis),
                PendingEvent::Gesture(gesture) => self.gestured(gesture),
                PendingEvent::Touch { touched, time } => self.touched(touched, time),
                PendingEvent::Tool {
                    device,
                    tool,
                    event,
                    state,
                    time,
                } => {
                    let relay =
                        input_ctx!(self).handle_tool_event(device, tool, event, &state, time);
                    if let Some(relay) = relay {
                        self.relay_tool(relay);
                    }
                }
                PendingEvent::Pad {
                    device,
                    input,
                    group,
                    mode,
                    time,
                } => switcher_ctx!(self).handle_pad_input(device, input, (group, mode), time),
                PendingEvent::Added(device) => switcher_ctx!(self).tablet_added(&device),
                PendingEvent::Removed(device) => switcher_ctx!(self).tablet_removed(device),
            }
        }
        input_ctx!(self).show_lock_leds();
    }

    fn pointer_moved(
        &mut self,
        target: Point<f64, Logical>,
        moved: Option<Moved>,
        placed: bool,
        time: Duration,
    ) {
        let to = person!(self).constrain(target);
        if placed && to == target {
            input_ctx!(self).cursor_follows(to);
        } else {
            input_ctx!(self).cursor_to(to);
        }
        let under = input_ctx!(self).pointing_at(to);
        windows_ctx!(self).follow_pointer(to);
        let mut pointer = self.person();
        pointer.activity();
        pointer.motion(Motion {
            location: to,
            under,
            time,
        });
        if let Some(moved) = moved {
            pointer.relative_motion(RelativeMotion {
                delta: moved.delta,
                unaccelerated: moved.unaccelerated,
                time,
            });
        }
        pointer.frame();
        input_ctx!(self).caption_follows_pointer(to);
        windows_ctx!(self).x11_drag_follows(time);
    }

    fn pointer_button(&mut self, code: u32, press: Press, time: Duration) {
        let seat = self.input.seat;
        let grabbed = self
            .seats
            .get(seat)
            .is_some_and(|seat_state| seat_state.pointer().grabbed());
        if press == Press::Pressed && !grabbed {
            let at = self.cursor.position();
            if windows_ctx!(self).snap_assist_pressed(at) {
                return;
            }
            windows_ctx!(self).focus_under(at);
            if !windows_ctx!(self).press_to_drag(code) {
                windows_ctx!(self).press_caption(code, at, time);
            }
        }
        let mut pointer = self.person();
        pointer.activity();
        pointer.button(Button { code, press, time });
        pointer.frame();
        windows_ctx!(self).x11_drag_follows(time);
    }

    fn pointer_axis(&mut self, axis: Axis) {
        person!(self).activity();
        if windows_ctx!(self).wheel_turns_workspaces(&axis) {
            return;
        }
        let mut pointer = self.person();
        pointer.axis(axis);
        pointer.frame();
    }
}

impl InputRef<'_> {
    pub(crate) fn element_under(&self, point: Point<f64, Logical>) -> Option<Under> {
        self.windows
            .desktop
            .element_under(&self.hit_surfaces(), point, |output| {
                around(self.windows, self.surfaces, output)
            })
    }

    pub(crate) fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(SurfaceId, Point<f64, Logical>)> {
        self.element_under(point)
            .map(|under| (under.surface, point - under.local))
    }

    pub(crate) fn focus_at(&self, point: Point<f64, Logical>) -> Option<Focus> {
        let under = self.element_under(point)?;
        if let Target::Window(window) = self.windows.desktop.root_of(under.target)
            && windows_ref!(self).modal_blocker(window).is_some()
        {
            return None;
        }
        Some(Focus::new(under.surface, point - under.local))
    }

    pub(crate) fn pointed_window(&self, location: Point<f64, Logical>) -> Option<WindowId> {
        let hit = self.hit_under(location)?;
        let Target::Window(window) = self.windows.desktop.root_of(hit.target()) else {
            return None;
        };
        self.windows
            .desktop
            .window(window)
            .is_some_and(|record| !record.unmanaged())
            .then_some(window)
    }
}

impl InputCtx<'_> {
    fn cursor_follows(&mut self, to: Point<f64, Logical>) -> bool {
        let from = self.cursor.move_to(to);
        if from == to {
            return false;
        }
        let drawn_in_software = self.cursor.surface().is_some();
        for position in [from, to] {
            if let Some(output) = self.outputs.containing(position)
                && (drawn_in_software || !output.has_cursor_plane())
            {
                self.frames.request(output.id());
            }
        }
        true
    }

    pub(super) fn cursor_to(&mut self, to: Point<f64, Logical>) {
        if self.cursor_follows(to) {
            self.backend.pointer_warped(&mut parts!(self));
        }
    }
}

impl WindowsCtx<'_> {
    pub(super) fn focus_under(&mut self, location: Point<f64, Logical>) {
        let Some(hit) = input_ref!(self).hit_under(location) else {
            return;
        };
        match self.windows.desktop.root_of(hit.target()) {
            Target::Window(window) => {
                self.interrupt_switch();
                let loose = self
                    .windows
                    .desktop
                    .window(window)
                    .is_none_or(|record| record.unmanaged());
                if !loose {
                    self.focus_window(Some(window));
                }
            }
            Target::Layer(layer) => {
                let Some(record) = self.windows.desktop.layer(layer) else {
                    return;
                };
                if record.keyboard_interactivity() != KeyboardInteractivity::None {
                    let surface = record.surface();
                    self.lend_keyboard(surface);
                }
            }
            Target::Lock(lock) => {
                let surface = self.shell.lock_surface(lock).map(|state| state.surface());
                if let Some(surface) = surface {
                    self.focus_surface(surface);
                }
            }
            Target::Popup(_) => {}
        }
    }

    pub(super) fn follow_pointer(&mut self, location: Point<f64, Logical>) {
        if !self.windows.rules().mode.focus_follows_pointer() || self.windows.desktop.locked() {
            return;
        }
        let Some(window) = input_ref!(self).pointed_window(location) else {
            return;
        };
        if windows_ref!(self).focused_window() == Some(window) {
            return;
        }
        let held = self.seats.get(self.input.seat).is_some_and(|seat_state| {
            seat_state.pointer().grabbed()
                || matches!(
                    seat_state.keyboard_grab(),
                    Some(KeyboardGrab::Xwayland { .. })
                )
        }) || self.windows.exclusive_layer_surface().is_some()
            || windows_ref!(self).layer_holds_focus()
            || self.input.grabs.popups.is_some()
            || self.switcher.switching();
        if !held {
            self.set_keyboard_window(Some(window));
            self.windows.recency.focused_by_pointer(window);
        }
    }
}
