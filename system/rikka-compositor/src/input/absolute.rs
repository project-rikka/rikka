use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_input::{DeviceId, Normalized};

use super::pending::{Pending, Spot};
use crate::context::InputRef;
use crate::outputs::Outputs;

pub(crate) fn place(
    outputs: &Outputs,
    pending: &Pending,
    device: DeviceId,
    position: Point<f64, Normalized>,
) -> Option<Point<f64, Logical>> {
    let named = pending.output_of(device);
    let output = named
        .and_then(|name| outputs.enabled().find(|output| output.name() == name))
        .or_else(|| outputs.enabled().next())?;
    let area = output.area()?.to_f64();
    Some(Point::new(
        area.loc.x + position.x.clamp(0.0, 1.0) * area.size.w,
        area.loc.y + position.y.clamp(0.0, 1.0) * area.size.h,
    ))
}

impl InputRef<'_> {
    pub(super) fn reach(&self, spot: Spot) -> Option<Point<f64, Logical>> {
        match spot {
            Spot::At(point) => Some(point),
            Spot::On { device, position } => {
                place(self.outputs, &self.input.pending, device, position)
            }
        }
    }
}
