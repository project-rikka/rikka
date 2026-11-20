use rikka_compositor_geometry::Point;
use rikka_compositor_types::{Press, ToolAxes};

use crate::space::Normalized;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEvent {
    ProximityIn,
    ProximityOut,
    Motion,
    Tip(Press),
    Button { code: u32, press: Press },
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) struct Changed: u8 {
        const PRESSURE = 1;
        const DISTANCE = 1 << 1;
        const TILT = 1 << 2;
        const ROTATION = 1 << 3;
        const SLIDER = 1 << 4;
        const WHEEL = 1 << 5;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolState {
    pub position: Point<f64, Normalized>,
    pub(crate) pressure: f32,
    pub(crate) distance: f32,
    pub(crate) tilt: (f32, f32),
    pub(crate) rotation: f32,
    pub(crate) slider: f32,
    pub(crate) wheel: f32,
    pub(crate) wheel_steps: i32,
    pub(crate) changed: Changed,
}

impl ToolState {
    #[must_use]
    pub fn axes(&self) -> ToolAxes {
        let has = |axis| self.changed.contains(axis);
        ToolAxes {
            pressure: has(Changed::PRESSURE).then_some(f64::from(self.pressure)),
            distance: has(Changed::DISTANCE).then_some(f64::from(self.distance)),
            tilt: has(Changed::TILT).then_some((f64::from(self.tilt.0), f64::from(self.tilt.1))),
            rotation: has(Changed::ROTATION).then_some(f64::from(self.rotation)),
            slider: has(Changed::SLIDER).then_some(f64::from(self.slider)),
            wheel: has(Changed::WHEEL).then_some((f64::from(self.wheel), self.wheel_steps)),
        }
    }
}
