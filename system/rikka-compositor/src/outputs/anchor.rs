use rikka_compositor_geometry::{Logical, Point};

use super::config::OutputConfig;

pub(crate) fn anchor<'a>(configs: impl IntoIterator<Item = &'a mut OutputConfig>) {
    let configs: Vec<&mut OutputConfig> = configs.into_iter().collect();
    let shift = configs
        .iter()
        .filter(|config| config.enabled)
        .map(|config| config.position)
        .fold(Point::zero(), outside);
    if shift == Point::zero() {
        return;
    }
    for config in configs {
        config.position -= shift;
    }
}

fn outside(shift: Point<i32, Logical>, position: Point<i32, Logical>) -> Point<i32, Logical> {
    Point::new(shift.x.min(position.x), shift.y.min(position.y))
}
