use rikka_types::{Rect, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    #[default]
    Stretch,
    Contain,
    Cover,
    Natural,
    ScaleDown,
}

impl ImageFit {
    #[must_use]
    pub fn place(self, bounds: Rect, natural: Size) -> Rect {
        if natural.is_empty() {
            return bounds;
        }
        let horizontal = bounds.width() / natural.width;
        let vertical = bounds.height() / natural.height;
        let scale = match self {
            ImageFit::Stretch => return bounds,
            ImageFit::Contain => horizontal.min(vertical),
            ImageFit::Cover => horizontal.max(vertical),
            ImageFit::Natural => 1.0,
            ImageFit::ScaleDown => horizontal.min(vertical).min(1.0),
        };
        Rect::from_center(bounds.center(), natural * scale)
    }
}
