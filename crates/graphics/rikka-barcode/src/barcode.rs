use rikka_types::{Point, Rect};

use crate::format::Format;

#[derive(Clone, Debug, PartialEq)]
pub struct Barcode {
    pub(crate) format: Format,
    pub(crate) text: String,
    pub(crate) corners: [Point; 4],
}

impl Barcode {
    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn corners(&self) -> [Point; 4] {
        self.corners
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        let [first, rest @ ..] = self.corners;
        let (low, high) = rest.iter().fold((first, first), |(low, high), &corner| {
            (low.min(corner), high.max(corner))
        });
        Rect::new(low.x, low.y, high.x - low.x, high.y - low.y)
    }
}
