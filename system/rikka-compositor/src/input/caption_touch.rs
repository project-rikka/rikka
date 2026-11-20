use std::time::Duration;

use rikka_compositor_caption::{Button, Part};
use rikka_compositor_desktop::WindowId;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{GrabFocus, TouchDown, TouchGrab, TouchHandle, TouchMotion, TouchUp};

use super::grabs::{Drag, Reshape};
use crate::context::{WindowsCtx, input_ref, person, windows_ctx};
use crate::state::Compositor;

const DRAG_SLOP: f64 = 8.0;

impl WindowsCtx<'_> {
    pub(crate) fn touch_caption(&mut self, slot: u32, at: Point<f64, Logical>, time: Duration) {
        if self.windows.desktop.locked() {
            return;
        }
        let Some((window, part)) = input_ref!(self).caption_at(at) else {
            return;
        };
        let held = match part {
            Part::Edge(edges) => match self.begin_reshape(window, edges, at) {
                Some(reshape) => Held::Reshaping(reshape),
                None => return,
            },
            Part::Title => {
                if self.windows.captions.title_pressed(window, at, time) {
                    return self.toggle_maximized(window);
                }
                Held::Title { from: at }
            }
            Part::Button(button) => {
                self.update_caption(|captions| {
                    captions.press(Some((window, button)));
                    captions.hover(Some((window, Part::Button(button))));
                });
                Held::Button {
                    button,
                    inside: true,
                }
            }
        };
        let reshaping = matches!(held, Held::Reshaping(_));
        person!(self).set_touch_grab(CaptionTouch { window, slot, held }, GrabFocus::Clear);
        if reshaping {
            self.input.grabs.requests.begin();
        }
    }
}

#[derive(Debug)]
enum Held {
    Title { from: Point<f64, Logical> },
    Moving(Drag),
    Reshaping(Reshape),
    Button { button: Button, inside: bool },
}

#[derive(Debug)]
struct CaptionTouch {
    window: WindowId,
    slot: u32,
    held: Held,
}

impl TouchGrab<Compositor> for CaptionTouch {
    fn down(&mut self, _touch: &mut TouchHandle<'_, Compositor>, _down: &TouchDown) {}

    fn motion(&mut self, touch: &mut TouchHandle<'_, Compositor>, motion: &TouchMotion) {
        if motion.slot != self.slot {
            return;
        }
        let state = touch.state();
        let at = motion.location;
        match &mut self.held {
            Held::Title { from } => {
                let travel = at - *from;
                if travel.x.hypot(travel.y) <= DRAG_SLOP {
                    return;
                }
                let from = *from;
                let Some(mut drag) = windows_ctx!(state).begin_drag(self.window, from) else {
                    return;
                };
                state.input.grabs.requests.begin();
                drag.follow(state, at);
                self.held = Held::Moving(drag);
            }
            Held::Moving(drag) => drag.follow(state, at),
            Held::Reshaping(reshape) => reshape.follow(state, at),
            Held::Button { button, inside } => {
                let part = Part::Button(*button);
                *inside = input_ref!(state).caption_at(at) == Some((self.window, part));
                let hovered = (*inside).then_some((self.window, part));
                windows_ctx!(state).update_caption(|captions| captions.hover(hovered));
            }
        }
    }

    fn up(&mut self, touch: &mut TouchHandle<'_, Compositor>, up: &TouchUp) {
        if up.slot != self.slot {
            return;
        }
        let state = touch.state();
        if let Held::Button {
            button,
            inside: true,
        } = self.held
        {
            windows_ctx!(state).caption_button_clicked(self.window, button);
        }
        touch.end_grab();
    }

    fn frame(&mut self, _touch: &mut TouchHandle<'_, Compositor>) {}

    fn cancel(&mut self, touch: &mut TouchHandle<'_, Compositor>) {
        if let Held::Button { inside, .. } = &mut self.held {
            *inside = false;
        }
        touch.end_grab();
    }

    fn ended(&mut self, state: &mut Compositor) {
        match &self.held {
            Held::Moving(drag) => drag.finish(state),
            Held::Reshaping(reshape) => reshape.finish(state),
            Held::Title { .. } => {}
            Held::Button { .. } => {
                windows_ctx!(state).update_caption(|captions| captions.press(None));
            }
        }
    }
}
