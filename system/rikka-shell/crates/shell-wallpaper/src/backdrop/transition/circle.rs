use rikka_types::{Point, Rect, RoundedRect};

use crate::backdrop::transition::{Stage, Transition};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Circle;

impl Transition for Circle {
    fn paint(&self, progress: f32, stage: &mut Stage<'_, '_>) {
        let bounds = stage.bounds();
        stage.outgoing().draw();

        let center = bounds.center();
        stage
            .incoming()
            .through(RoundedRect::circle(center, reach(bounds) * progress))
            .draw();
    }
}

fn reach(bounds: Rect) -> f32 {
    bounds
        .center()
        .distance(Point::new(bounds.min_x(), bounds.min_y()))
}
