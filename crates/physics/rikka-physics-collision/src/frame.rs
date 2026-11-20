use rikka_physics_math::{Aabb, Transform, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyFrame {
    pub transform: Transform,
    pub center: Vec3,
    pub reach: Vec3,
    pub fast: bool,
    pub speculative: f32,
    pub sweep: f32,
    pub motion: Vec3,
    pub turn: Vec3,
}

impl BodyFrame {
    pub fn swept(&self, bounds: Aabb) -> Aabb {
        let grown = bounds.inflated(self.sweep);
        grown.union(&grown.translated(self.motion))
    }
}

impl Default for BodyFrame {
    fn default() -> Self {
        Self {
            transform: Transform::IDENTITY,
            center: Vec3::ZERO,
            reach: Vec3::ZERO,
            speculative: 0.0,
            motion: Vec3::ZERO,
            turn: Vec3::ZERO,
            sweep: 0.0,
            fast: false,
        }
    }
}

#[inline]
pub fn rotation_arc(turn: Vec3, reach: Vec3) -> Vec3 {
    let turn = turn.abs();
    Vec3::new(
        turn.y * reach.z + turn.z * reach.y,
        turn.z * reach.x + turn.x * reach.z,
        turn.x * reach.y + turn.y * reach.x,
    )
}
