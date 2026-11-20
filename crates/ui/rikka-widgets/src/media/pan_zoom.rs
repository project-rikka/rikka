use rikka_types::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    Fit,
    Scale(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanZoom {
    extent: Size,
    view: Size,
    zoom: Zoom,
    center: Point,
    least: f32,
    most: f32,
    margin: f32,
}

impl PanZoom {
    #[must_use]
    pub const fn new(least: f32, most: f32) -> PanZoom {
        PanZoom {
            extent: Size::ZERO,
            view: Size::ZERO,
            zoom: Zoom::Fit,
            center: Point::ZERO,
            least,
            most,
            margin: 0.0,
        }
    }

    #[must_use]
    pub const fn margin(mut self, margin: f32) -> PanZoom {
        self.margin = margin;
        self
    }

    #[must_use]
    pub fn least(&self) -> f32 {
        self.least
    }

    #[must_use]
    pub fn most(&self) -> f32 {
        self.most
    }

    #[must_use]
    pub fn extent(&self) -> Size {
        self.extent
    }

    #[must_use]
    pub fn view(&self) -> Size {
        self.view
    }

    #[must_use]
    pub fn zoom(&self) -> Zoom {
        self.zoom
    }

    #[must_use]
    pub fn is_fitted(&self) -> bool {
        self.zoom == Zoom::Fit
    }

    pub fn show(&mut self, extent: Size) -> bool {
        if self.extent == extent {
            return false;
        }
        self.extent = extent;
        self.zoom = Zoom::Fit;
        true
    }

    pub fn meet(&mut self, view: Size) -> bool {
        let changed = self.view != view;
        self.view = view;
        changed
    }

    #[must_use]
    pub fn fitting(&self) -> f32 {
        if self.extent.is_empty() || self.view.is_empty() {
            return 1.0;
        }
        let room = Size::new(
            (self.view.width - self.margin * 2.0).max(1.0),
            (self.view.height - self.margin * 2.0).max(1.0),
        );
        (room.width / self.extent.width)
            .min(room.height / self.extent.height)
            .clamp(self.least, 1.0)
    }

    #[must_use]
    pub fn scale(&self) -> f32 {
        match self.zoom {
            Zoom::Fit => self.fitting(),
            Zoom::Scale(scale) => scale,
        }
    }

    #[must_use]
    pub fn center(&self) -> Point {
        match self.zoom {
            Zoom::Fit => self.middle(),
            Zoom::Scale(_) => self.center,
        }
    }

    #[must_use]
    pub fn to_content(&self, at: Point) -> Point {
        let (scale, center) = (self.scale(), self.center());
        Point::new(
            center.x + (at.x - self.view.width / 2.0) / scale,
            center.y + (at.y - self.view.height / 2.0) / scale,
        )
    }

    #[must_use]
    pub fn to_view(&self, point: Point) -> Point {
        let (scale, origin) = (self.scale(), self.origin());
        Point::new(origin.x + point.x * scale, origin.y + point.y * scale)
    }

    #[must_use]
    pub fn origin(&self) -> Point {
        let (scale, center) = (self.scale(), self.center());
        Point::new(
            self.view.width / 2.0 - center.x * scale,
            self.view.height / 2.0 - center.y * scale,
        )
    }

    #[must_use]
    pub fn placed(&self) -> Rect {
        let scale = self.scale();
        Rect::from_origin_size(
            self.origin(),
            Size::new(self.extent.width * scale, self.extent.height * scale),
        )
    }

    #[must_use]
    pub fn visible(&self) -> Rect {
        let from = self.to_content(Point::ZERO);
        let to = self.to_content(Point::new(self.view.width, self.view.height));
        Rect::new(from.x, from.y, to.x - from.x, to.y - from.y)
    }

    pub fn fit(&mut self) -> bool {
        self.set(Zoom::Fit, Point::ZERO)
    }

    pub fn reset(&mut self, zoom: Zoom) -> bool {
        let middle = self.middle();
        self.set(zoom, middle)
    }

    pub fn zoom_to(&mut self, scale: f32, anchor: Option<Point>) -> bool {
        let anchor =
            anchor.unwrap_or_else(|| Point::new(self.view.width / 2.0, self.view.height / 2.0));
        let anchored = self.to_content(anchor);
        let scale = scale.clamp(self.least, self.most);
        let center = Point::new(
            anchored.x - (anchor.x - self.view.width / 2.0) / scale,
            anchored.y - (anchor.y - self.view.height / 2.0) / scale,
        );
        self.set(Zoom::Scale(scale), center)
    }

    pub fn zoom_by(&mut self, factor: f32, anchor: Option<Point>) -> bool {
        self.zoom_to(self.scale() * factor, anchor)
    }

    pub fn step(&mut self, presets: &[f32], inward: bool, anchor: Option<Point>) -> bool {
        let scale = self.scale();
        let next = match inward {
            true => presets.iter().find(|preset| **preset > scale * 1.001),
            false => presets.iter().rev().find(|preset| **preset < scale / 1.001),
        };
        next.is_some_and(|next| self.zoom_to(*next, anchor))
    }

    pub fn frame(&mut self, from: Point, to: Point) -> bool {
        let width = (to.x - from.x).abs().max(1.0);
        let height = (to.y - from.y).abs().max(1.0);
        let scale = (self.view.width / width)
            .min(self.view.height / height)
            .clamp(self.least, self.most);
        let middle = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        self.set(Zoom::Scale(scale), middle)
    }

    pub fn pan(&mut self, by: Point) -> bool {
        let (scale, center) = (self.scale(), self.center());
        let moved = Point::new(center.x - by.x / scale, center.y - by.y / scale);
        self.set(Zoom::Scale(scale), moved)
    }

    #[must_use]
    pub fn can_zoom(&self, inward: bool) -> bool {
        match inward {
            true => self.scale() < self.most,
            false => self.scale() > self.least,
        }
    }

    fn middle(&self) -> Point {
        Point::new(self.extent.width / 2.0, self.extent.height / 2.0)
    }

    fn set(&mut self, zoom: Zoom, center: Point) -> bool {
        let unchanged = match (self.zoom, zoom) {
            (Zoom::Fit, Zoom::Fit) => true,
            _ => self.zoom == zoom && self.center == center,
        };
        if unchanged {
            return false;
        }
        self.zoom = zoom;
        self.center = center;
        true
    }
}
