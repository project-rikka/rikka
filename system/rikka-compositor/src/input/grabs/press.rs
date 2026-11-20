use rikka_compositor_desktop::{Target, WindowId};
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::Focus;
use rikka_input::evdev::{BTN_LEFT, BTN_RIGHT};
use rikka_types::Modifiers;

use crate::context::WindowsCtx;
use crate::context::{InputCtx, InputRef, input_ref, windows_ctx};
use crate::input::keys::modifiers;
use rikka_compositor_tiling::Edges;

const DRAG_MODIFIER: Modifiers = Modifiers::LOGO;

impl InputCtx<'_> {
    pub(crate) fn pointing_at(&mut self, at: Point<f64, Logical>) -> Option<Focus> {
        if self.input.grabs.requests.is_active() {
            return None;
        }
        windows_ctx!(self).snap_assist_hovered(at);
        if self.snap.snap_assist_covers(at) {
            return None;
        }
        input_ref!(self).focus_at(at)
    }
}

impl InputRef<'_> {
    fn is_drag_modifier_pressed(&self) -> bool {
        self.seats
            .get(self.input.seat)
            .is_some_and(|seat| modifiers(seat.keyboard().modifiers()) == DRAG_MODIFIER)
    }

    fn movable_under(&self, at: Point<f64, Logical>) -> Option<WindowId> {
        let hit = self.hit_under(at)?;
        let Target::Window(window) = self.windows.desktop.root_of(hit.target()) else {
            return None;
        };
        let record = self.windows.desktop.window(window)?;
        (!record.unmanaged() && !self.windows.enlargements(window).fullscreen).then_some(window)
    }
}

impl WindowsCtx<'_> {
    pub(crate) fn press_to_drag(&mut self, button: u32) -> bool {
        if self.windows.desktop.locked() {
            return false;
        }
        let at = self.cursor.position();
        if input_ref!(self).is_drag_modifier_pressed()
            && let Some(window) = input_ref!(self).movable_under(at)
        {
            match button {
                BTN_LEFT => {
                    self.start_move(window, at);
                    return true;
                }
                BTN_RIGHT => {
                    let edges = self
                        .windows
                        .desktop
                        .window(window)
                        .map_or(Edges::BOTTOM_RIGHT, |record| {
                            Edges::quadrant(record.geometry(), at)
                        });
                    self.start_resize(window, edges, at);
                    return true;
                }
                _ => return false,
            }
        }
        if button != BTN_LEFT {
            return false;
        }
        if let Some((group, divider)) = self.snap_divider_at(at) {
            self.start_divider(group, divider, at);
            return true;
        }
        if let Some((window, edges)) = self.windows.tile_edge_at(at)
            && !self.windows.enlargements(window).fullscreen
        {
            self.start_resize(window, edges, at);
            return true;
        }
        false
    }
}
