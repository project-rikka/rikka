use rikka_compositor_geometry::{Buffer, Damage, Logical};

#[derive(Debug, Clone, Default)]
pub struct SurfaceDamage {
    pub buffer: Damage<Buffer>,
    pub surface: Damage<Logical>,
}

impl SurfaceDamage {
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty() && self.surface.is_empty()
    }
}
