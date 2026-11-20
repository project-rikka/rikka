use std::cell::RefCell;
use std::rc::Rc;

use rikka_types::{Point, Rect, Size};
use rikka_ui::{Observed, Observers};
use rikka_widgets::PanZoom;

const PRESETS: [f32; 27] = [
    0.01, 0.02, 0.03, 0.05, 0.1, 0.15, 0.2, 0.25, 0.333, 0.5, 0.667, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0,
    6.0, 7.0, 8.0, 10.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0,
];
pub(crate) const LEAST: f32 = PRESETS[0];
pub(crate) const MOST: f32 = PRESETS[PRESETS.len() - 1];
const FIT_MARGIN: f32 = 24.0;

#[derive(Clone, Debug)]
pub(crate) struct Camera(Rc<Inner>);

#[derive(Debug)]
struct Inner {
    pan_zoom: RefCell<PanZoom>,
    observers: Observers,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera(Rc::new(Inner {
            pan_zoom: RefCell::new(PanZoom::new(LEAST, MOST).margin(FIT_MARGIN)),
            observers: Observers::new(),
        }))
    }
}

impl Camera {
    pub(crate) fn same(&self, other: &Camera) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn read(&self) -> PanZoom {
        self.0.observers.read();
        *self.0.pan_zoom.borrow()
    }

    fn change(&self, change: impl FnOnce(&mut PanZoom) -> bool) {
        let changed = change(&mut self.0.pan_zoom.borrow_mut());
        if changed {
            self.0.observers.changed();
        }
    }

    pub(crate) fn show(&self, extent: Size) {
        self.change(|pan_zoom| pan_zoom.show(extent));
    }

    pub(crate) fn extent(&self) -> Size {
        self.read().extent()
    }

    pub(crate) fn meet(&self, viewport: Size) {
        self.change(|pan_zoom| pan_zoom.meet(viewport) && pan_zoom.is_fitted());
    }

    pub(crate) fn scale(&self) -> f32 {
        self.read().scale()
    }

    pub(crate) fn picture_point(&self, at: Point) -> Point {
        self.read().to_content(at)
    }

    pub(crate) fn visible_area(&self) -> Rect {
        self.read().visible()
    }

    pub(crate) fn origin(&self) -> Point {
        self.read().origin()
    }

    pub(crate) fn fit(&self) {
        self.change(PanZoom::fit);
    }

    pub(crate) fn zoom_to(&self, scale: f32, anchor: Option<Point>) {
        self.change(|pan_zoom| pan_zoom.zoom_to(scale, anchor));
    }

    pub(crate) fn step(&self, inward: bool, anchor: Option<Point>) {
        self.change(|pan_zoom| pan_zoom.step(&PRESETS, inward, anchor));
    }

    pub(crate) fn magnify(&self, factor: f32, anchor: Point) {
        self.change(|pan_zoom| pan_zoom.zoom_by(factor, Some(anchor)));
    }

    pub(crate) fn frame(&self, from: Point, to: Point) {
        self.change(|pan_zoom| pan_zoom.frame(from, to));
    }

    pub(crate) fn pan(&self, by: Point) {
        self.change(|pan_zoom| pan_zoom.pan(by));
    }

    pub(crate) fn can_zoom(&self, inward: bool) -> bool {
        self.read().can_zoom(inward)
    }
}

impl Observed for Camera {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}
