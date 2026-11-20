use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_tiling::Edges;
use rikka_theme::caption;

use crate::button::Button;
use crate::part::Part;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caption {
    size: Size<f64, Logical>,
    reach: f64,
    resizable: bool,
    maximized: bool,
}

impl Caption {
    pub const BAR: i32 = caption::BAR as i32;

    pub const REACH: i32 = 8;

    #[must_use]
    pub fn new(size: Size<i32, Logical>) -> Self {
        Self {
            size: size.to_f64(),
            reach: 0.0,
            resizable: true,
            maximized: false,
        }
    }

    #[must_use]
    pub fn with_reach(mut self, reach: i32) -> Self {
        self.reach = f64::from(reach.max(0));
        self
    }

    #[must_use]
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    #[must_use]
    pub fn with_maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    pub fn buttons(&self) -> impl Iterator<Item = (Button, Rect<f64, Logical>)> + '_ {
        let shown: [Option<Button>; 3] = [
            Some(Button::Minimize),
            self.resizable.then_some(match self.maximized {
                true => Button::Restore,
                false => Button::Maximize,
            }),
            Some(Button::Close),
        ];
        let count = shown.iter().flatten().count();
        let width = f64::from(caption::BUTTON_WIDTH);
        let start = self.size.w - width * count as f64;
        shown
            .into_iter()
            .flatten()
            .enumerate()
            .map(move |(index, button)| {
                let rect = Rect::new(
                    Point::new(start + width * index as f64, 0.0),
                    Size::new(width, f64::from(caption::HEIGHT)),
                );
                (button, rect)
            })
    }

    #[must_use]
    pub fn bar(&self) -> Rect<f64, Logical> {
        Rect::from_size(Size::new(self.size.w, f64::from(caption::HEIGHT)))
    }

    #[must_use]
    pub fn separator(&self) -> Rect<f64, Logical> {
        Rect::new(
            Point::new(0.0, f64::from(caption::HEIGHT)),
            Size::new(self.size.w, f64::from(caption::SEPARATOR)),
        )
    }

    #[must_use]
    pub fn band(&self) -> Rect<f64, Logical> {
        Rect::from_size(Size::new(self.size.w, f64::from(Self::BAR)))
    }

    #[must_use]
    pub fn title(&self) -> Rect<f64, Logical> {
        let inset = f64::from(caption::TITLE_INSET);
        let buttons = self.buttons().count() as f64 * f64::from(caption::BUTTON_WIDTH);
        Rect::new(
            Point::new(inset, 0.0),
            Size::new(
                (self.size.w - inset * 2.0 - buttons).max(0.0),
                f64::from(caption::HEIGHT),
            ),
        )
    }

    #[must_use]
    pub fn part_at(&self, at: Point<f64, Logical>) -> Option<Part> {
        let bar = f64::from(Self::BAR);
        let inside = at.x >= 0.0 && at.x < self.size.w && at.y >= 0.0 && at.y < self.size.h;
        if inside && at.y >= bar {
            return None;
        }
        if let Some(edges) = self.edges_at(at) {
            return Some(Part::Edge(edges));
        }
        if !inside {
            return None;
        }
        self.buttons()
            .find(|(_, rect)| rect.contains(at))
            .map_or(Some(Part::Title), |(button, _)| Some(Part::Button(button)))
    }

    fn edges_at(&self, at: Point<f64, Logical>) -> Option<Edges> {
        if self.reach <= 0.0 {
            return None;
        }
        let left = at.x + self.reach;
        let right = self.size.w + self.reach - at.x;
        let top = at.y + self.reach;
        let bottom = self.size.h + self.reach - at.y;
        if left < 0.0 || right < 0.0 || top < 0.0 || bottom < 0.0 {
            return None;
        }
        let border = self.reach + f64::from(caption::RESIZE_BORDER);
        let corner = self.reach + f64::from(caption::RESIZE_CORNER);
        let horizontal = near(left, right, border, Edges::LEFT, Edges::RIGHT);
        let vertical = near(top, bottom, border, Edges::TOP, Edges::BOTTOM);
        if horizontal.is_none() && vertical.is_none() {
            return None;
        }
        let horizontal =
            horizontal.or_else(|| near(left, right, corner, Edges::LEFT, Edges::RIGHT));
        let vertical = vertical.or_else(|| near(top, bottom, corner, Edges::TOP, Edges::BOTTOM));
        Some(horizontal.unwrap_or_else(Edges::empty) | vertical.unwrap_or_else(Edges::empty))
    }
}

fn near(low: f64, high: f64, reach: f64, toward_low: Edges, toward_high: Edges) -> Option<Edges> {
    if low <= reach {
        Some(toward_low)
    } else if high <= reach {
        Some(toward_high)
    } else {
        None
    }
}
