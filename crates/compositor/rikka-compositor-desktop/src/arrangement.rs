use rikka_compositor_geometry::{Logical, Size};
use rikka_compositor_shell::LayerId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerConfigure {
    pub layer: LayerId,
    pub size: Size<i32, Logical>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arrangement {
    pub configures: Vec<LayerConfigure>,
    pub closed: Vec<LayerId>,
    pub work_area_changed: bool,
}
