use std::f32::consts::PI;

use crate::codec::{record, within};

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSplit {
    pub split_angle: f32,
    pub use_edge_angle: bool,
    pub use_edge_sharp: bool,
}

impl Default for EdgeSplit {
    fn default() -> EdgeSplit {
        EdgeSplit {
            split_angle: 30.0f32.to_radians(),
            use_edge_angle: true,
            use_edge_sharp: true,
        }
    }
}

record!(EdgeSplit {
    split_angle => within(0.0, PI, "edge split angle is outside 0..180 degrees"),
    use_edge_angle,
    use_edge_sharp,
});
