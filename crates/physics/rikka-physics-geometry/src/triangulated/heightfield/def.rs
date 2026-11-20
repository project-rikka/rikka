use rikka_physics_math::Vec3;

#[must_use]
#[derive(Clone, Copy, Debug)]
pub struct HeightFieldDef<'a> {
    pub(crate) heights: &'a [f32],
    pub(crate) materials: &'a [u8],
    pub(crate) columns: u32,
    pub(crate) rows: u32,
    pub(crate) scale: Vec3,
    pub(crate) height_range: Option<(f32, f32)>,
    pub(crate) clockwise: bool,
}

impl<'a> HeightFieldDef<'a> {
    pub fn new(heights: &'a [f32], columns: u32, rows: u32) -> Self {
        Self {
            heights,
            materials: &[],
            columns,
            rows,
            scale: Vec3::ONE,
            height_range: None,
            clockwise: false,
        }
    }

    pub fn materials(mut self, materials: &'a [u8]) -> Self {
        self.materials = materials;
        self
    }

    pub fn scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    pub fn height_range(mut self, minimum: f32, maximum: f32) -> Self {
        self.height_range = Some((minimum.min(maximum), minimum.max(maximum)));
        self
    }

    pub fn clockwise(mut self, clockwise: bool) -> Self {
        self.clockwise = clockwise;
        self
    }
}
