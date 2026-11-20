use rikka_compositor_geometry::{Logical, Point, Transform};
use rikka_compositor_planes::VrrPolicy;

use super::head::Mode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OutputConfig {
    pub(crate) enabled: bool,
    pub(crate) mode: Mode,
    pub(crate) scale: f64,
    pub(crate) transform: Transform,
    pub(crate) position: Point<i32, Logical>,
    pub(crate) adaptive_sync: VrrPolicy,
    pub(crate) hdr: bool,
    pub(crate) sdr_brightness: f64,
}
