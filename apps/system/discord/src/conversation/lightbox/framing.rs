use rikka_app::prelude::{PanZoom, Point, Rect, Size};

const LEAST: f32 = 0.02;
const MOST: f32 = 16.0;
const STEP: f32 = 1.25;
const DETAIL: f32 = 2.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Framing {
    pan_zoom: PanZoom,
    room: Rect,
    bounds: Size,
}

impl Framing {
    pub(super) fn new(natural: Size) -> Framing {
        let mut pan_zoom = PanZoom::new(LEAST, MOST);
        pan_zoom.show(natural);
        Framing {
            pan_zoom,
            room: Rect::default(),
            bounds: Size::ZERO,
        }
    }

    pub(super) fn show(&mut self, natural: Size) {
        self.pan_zoom.show(natural);
    }

    pub(super) fn fit_into(&mut self, bounds: Size, room: Rect) -> bool {
        let resized = self.bounds != bounds;
        self.bounds = bounds;
        self.room = room;
        self.pan_zoom.meet(room.size);
        resized
    }

    pub(super) fn is_zoomed(&self) -> bool {
        !self.pan_zoom.is_fitted()
    }

    pub(super) fn picture(&self) -> Rect {
        self.pan_zoom.placed().translate(self.room.origin)
    }

    pub(super) fn contains(&self, point: Point) -> bool {
        self.picture().contains(point)
    }

    pub(super) fn overflows(&self) -> bool {
        let size = self.pan_zoom.placed().size;
        size.width > self.bounds.width || size.height > self.bounds.height
    }

    pub(super) fn scale(&self) -> f32 {
        self.pan_zoom.scale()
    }

    pub(super) fn toggle(&mut self, anchor: Point) {
        if self.pan_zoom.is_fitted() {
            let detail = (self.pan_zoom.fitting() * DETAIL).max(1.0);
            self.pan_zoom.zoom_to(detail, Some(self.in_room(anchor)));
        } else {
            self.pan_zoom.fit();
        }
    }

    pub(super) fn fit(&mut self) {
        self.pan_zoom.fit();
    }

    pub(super) fn actual_size(&mut self) {
        self.pan_zoom.zoom_to(1.0, None);
    }

    pub(super) fn zoom_in(&mut self) {
        self.pan_zoom.zoom_by(STEP, None);
    }

    pub(super) fn zoom_out(&mut self) {
        self.pan_zoom.zoom_by(1.0 / STEP, None);
    }

    pub(super) fn notches(&mut self, notches: f32, anchor: Point) {
        self.pan_zoom
            .zoom_by(STEP.powf(notches), Some(self.in_room(anchor)));
    }

    pub(super) fn pinched(&mut self, from: f32, scale: f32, anchor: Point) {
        self.pan_zoom
            .zoom_to(from * scale, Some(self.in_room(anchor)));
    }

    pub(super) fn pan(&mut self, by: Point) {
        self.pan_zoom.pan(by);
    }

    fn in_room(&self, point: Point) -> Point {
        point - self.room.origin
    }
}
