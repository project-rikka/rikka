use rikka_compositor_desktop::Drawn;
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_seat::SeatId;
use rikka_compositor_surface::{SurfaceId, Surfaces};

use super::origin::surface_origin;
use crate::context::{InputCtx, input_ref, seat_parts};
use crate::windows::Windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlacedPopup {
    popup: SurfaceId,
    parent: SurfaceId,
    at: Point<i32, Logical>,
}

#[derive(Debug, Default)]
pub(crate) struct InputPopups {
    shown: Vec<PlacedPopup>,
}

impl InputPopups {
    pub(crate) fn draw(&self, drawn: &mut Vec<Drawn>, windows: &Windows, surfaces: &Surfaces) {
        for shown in &self.shown {
            let parent = surfaces.root(shown.parent);
            let Some(target) = drawn.iter().find_map(|item| match *item {
                Drawn::Surface { target, root, .. } if root == parent => Some(target),
                _ => None,
            }) else {
                continue;
            };
            let Some(origin) = surface_origin(windows, surfaces, shown.parent) else {
                continue;
            };
            drawn.push(Drawn::Surface {
                target,
                root: shown.popup,
                location: origin + shown.at,
            });
        }
    }

    fn forget(&mut self, popup: SurfaceId) -> bool {
        let before = self.shown.len();
        self.shown.retain(|shown| shown.popup != popup);
        self.shown.len() != before
    }
}

pub(crate) fn candidates(
    caret: Rect<i32, Logical>,
    size: Size<i32, Logical>,
    screen: Rect<i32, Logical>,
) -> Point<i32, Logical> {
    let below = caret.loc.y + caret.size.h;
    let y = if below + size.h <= screen.loc.y + screen.size.h {
        below
    } else {
        (caret.loc.y - size.h).max(screen.loc.y)
    };
    let x = caret
        .loc
        .x
        .min(screen.loc.x + screen.size.w - size.w)
        .max(screen.loc.x);
    Point::new(x, y)
}

impl InputCtx<'_> {
    pub(super) fn input_popup_gone(&mut self, popup: SurfaceId) {
        if self.input.popups.forget(popup) {
            self.frames.request_all();
        }
    }

    pub(super) fn input_popup_shown(
        &mut self,
        seat: SeatId,
        popup: SurfaceId,
        parent: Option<SurfaceId>,
        cursor: Option<Rect<i32, Logical>>,
    ) {
        let Some(parent) = parent else {
            return self.input_popup_gone(popup);
        };
        let caret = cursor.unwrap_or_default();
        let under = Point::new(caret.loc.x, caret.loc.y + caret.size.h);
        let size = self
            .surfaces
            .bounding_box(popup)
            .map_or_else(Size::default, |bounds| bounds.size);
        let at = input_ref!(self)
            .surface_origin(parent)
            .and_then(|origin| {
                let caret = Rect::new(origin + caret.loc, caret.size);
                let screen = self.outputs.containing(caret.loc.to_f64())?.area()?;
                Some(candidates(caret, size, screen) - origin)
            })
            .unwrap_or(under);
        seat_parts!(self)
            .into_seat(seat)
            .place_input_popup(popup, at);
        let shown = PlacedPopup { popup, parent, at };
        let popups = &mut self.input.popups.shown;
        match popups.iter_mut().find(|placed| placed.popup == popup) {
            Some(placed) => *placed = shown,
            None => popups.push(shown),
        }
        self.frames.request_all();
    }
}
