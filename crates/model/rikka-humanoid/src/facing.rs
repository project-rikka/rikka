use rikka_model_core::glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Facing {
    pub(crate) forward: Vec3,
    left: Vec3,
    midline: Vec3,
}

impl Facing {
    pub(crate) fn new(forward: Vec3, midline: Vec3) -> Facing {
        Facing {
            forward,
            left: Vec3::Y.cross(forward).normalize_or(Vec3::X),
            midline,
        }
    }

    pub(crate) fn lateral(&self, at: Vec3) -> f32 {
        (at - self.midline).dot(self.left)
    }

    pub(crate) fn place(&self, at: Vec3) -> Vec3 {
        let offset = at - self.midline;
        Vec3::new(offset.dot(self.left), offset.y, offset.dot(self.forward))
    }
}
