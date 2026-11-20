use std::time::Duration;

use rikka_compositor_caption::{Button, Part};
use rikka_compositor_desktop::{Hit, WindowId};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{
    Button as Pressed, CursorShape, GrabFocus, Motion, PointerGrab, PointerHandle,
};
use rikka_compositor_tiling::Edges;
use rikka_compositor_types::Press;
use rikka_input::evdev::BTN_LEFT;

use super::grabs::release;
use crate::context::{
    InputCtx, InputRef, WindowsCtx, input_ref, parts, person, windows_ctx, windows_ref,
};
use crate::state::Compositor;
use crate::windows::around;

const DRAG_SLOP: f64 = 4.0;

impl InputRef<'_> {
    pub(crate) fn hit_under(&self, at: Point<f64, Logical>) -> Option<Hit> {
        self.windows
            .desktop
            .hit_under(&self.hit_surfaces(), at, |output| {
                around(self.windows, self.surfaces, output)
            })
    }

    pub(crate) fn caption_at(&self, at: Point<f64, Logical>) -> Option<(WindowId, Part)> {
        let Hit::Frame { window, local } = self.hit_under(at)? else {
            return None;
        };
        if windows_ref!(self).modal_blocker(window).is_some() {
            return None;
        }
        let part = self.windows.caption_of(window)?.part_at(local)?;
        Some((window, part))
    }
}

impl InputCtx<'_> {
    pub(crate) fn caption_follows_pointer(&mut self, at: Point<f64, Logical>) {
        let Some(pointer) = self.seats.get(self.input.seat).map(|seat| seat.pointer()) else {
            return;
        };
        if pointer.grabbed() || self.windows.captions.pressed().is_some() {
            return;
        }
        let free = pointer.buttons().is_empty();
        let hovered = free.then(|| input_ref!(self).caption_at(at)).flatten();
        windows_ctx!(self).update_caption(|captions| captions.hover(hovered));
        let shape = match hovered {
            Some((_, Part::Edge(edges))) => Some(resize_cursor(edges)),
            Some((_, Part::Title | Part::Button(_))) | None => None,
        };
        if self.cursor.decorate(shape) {
            self.backend.cursor_changed(&mut parts!(self));
            self.frames.request_all();
        }
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn press_caption(&mut self, code: u32, at: Point<f64, Logical>, time: Duration) {
        if code != BTN_LEFT || self.windows.desktop.locked() {
            return;
        }
        let Some((window, part)) = input_ref!(self).caption_at(at) else {
            return;
        };
        match part {
            Part::Edge(edges) => self.start_resize(window, edges, at),
            Part::Title => {
                if self.windows.captions.title_pressed(window, at, time) {
                    return self.toggle_maximized(window);
                }
                person!(self).set_pointer_grab(TitleGrab { window, from: at }, GrabFocus::Clear);
            }
            Part::Button(button) => {
                self.update_caption(|captions| {
                    captions.press(Some((window, button)));
                    captions.hover(Some((window, Part::Button(button))));
                });
                person!(self).set_pointer_grab(
                    ButtonGrab {
                        window,
                        button,
                        released_inside: false,
                    },
                    GrabFocus::Clear,
                );
            }
        }
    }
}

#[derive(Debug)]
struct TitleGrab {
    window: WindowId,
    from: Point<f64, Logical>,
}

impl PointerGrab<Compositor> for TitleGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, Compositor>, motion: &Motion) {
        pointer.motion(&Motion {
            under: None,
            ..*motion
        });
        let travel = motion.location - self.from;
        if travel.x.hypot(travel.y) > DRAG_SLOP {
            let state = pointer.state();
            windows_ctx!(state).start_move(self.window, self.from);
        }
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, Compositor>, button: &Pressed) {
        release(pointer, button);
    }
}

#[derive(Debug)]
struct ButtonGrab {
    window: WindowId,
    button: Button,
    released_inside: bool,
}

impl ButtonGrab {
    fn inside(&self, state: &Compositor, at: Point<f64, Logical>) -> bool {
        input_ref!(state).caption_at(at) == Some((self.window, Part::Button(self.button)))
    }
}

impl PointerGrab<Compositor> for ButtonGrab {
    fn motion(&mut self, pointer: &mut PointerHandle<'_, Compositor>, motion: &Motion) {
        pointer.motion(&Motion {
            under: None,
            ..*motion
        });
        let state = pointer.state();
        let hovered = self
            .inside(state, motion.location)
            .then_some((self.window, Part::Button(self.button)));
        windows_ctx!(state).update_caption(|captions| captions.hover(hovered));
    }

    fn button(&mut self, pointer: &mut PointerHandle<'_, Compositor>, button: &Pressed) {
        if button.press == Press::Released && button.code == BTN_LEFT {
            let at = pointer.location();
            self.released_inside = self.inside(pointer.state(), at);
        }
        release(pointer, button);
    }

    fn ended(&mut self, state: &mut Compositor) {
        windows_ctx!(state).update_caption(|captions| captions.press(None));
        if self.released_inside {
            windows_ctx!(state).caption_button_clicked(self.window, self.button);
        }
    }
}

fn resize_cursor(edges: Edges) -> CursorShape {
    match edges {
        Edges::TOP_LEFT => CursorShape::NwResize,
        Edges::TOP_RIGHT => CursorShape::NeResize,
        Edges::BOTTOM_LEFT => CursorShape::SwResize,
        Edges::BOTTOM_RIGHT => CursorShape::SeResize,
        Edges::TOP => CursorShape::NResize,
        Edges::BOTTOM => CursorShape::SResize,
        Edges::LEFT => CursorShape::WResize,
        Edges::RIGHT => CursorShape::EResize,
        _ => CursorShape::Default,
    }
}
