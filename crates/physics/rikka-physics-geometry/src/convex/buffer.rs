use rikka_physics_math::Vec3;

use crate::ShapeProxy;

pub const PROXY_BUFFER_POINTS: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct ProxyBuffer {
    points: [Vec3; PROXY_BUFFER_POINTS],
}

impl Default for ProxyBuffer {
    fn default() -> Self {
        Self {
            points: [Vec3::ZERO; PROXY_BUFFER_POINTS],
        }
    }
}

impl ProxyBuffer {
    pub(crate) fn fill(
        &mut self,
        points: impl IntoIterator<Item = Vec3>,
        radius: f32,
    ) -> ShapeProxy<'_> {
        let mut count = 0;
        for (slot, point) in self.points.iter_mut().zip(points) {
            *slot = point;
            count += 1;
        }
        ShapeProxy::new(&self.points[..count], radius)
    }
}
