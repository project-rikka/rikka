use rikka_physics_math::{Vec3, Vec3W, Wide};

pub(crate) const IMPULSE_SLOTS: usize = 30;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct JointImpulses(pub [f32; IMPULSE_SLOTS]);

impl JointImpulses {
    pub(crate) const ZERO: Self = Self([0.0; IMPULSE_SLOTS]);

    #[inline]
    pub(crate) fn vec3(&self, slot: usize) -> Vec3 {
        Vec3::new(self.0[slot], self.0[slot + 1], self.0[slot + 2])
    }
}

#[inline(always)]
pub(crate) fn put_vec3<W: Wide>(out: &mut [W; IMPULSE_SLOTS], slot: usize, value: Vec3W<W>) {
    out[slot] = value.x;
    out[slot + 1] = value.y;
    out[slot + 2] = value.z;
}
