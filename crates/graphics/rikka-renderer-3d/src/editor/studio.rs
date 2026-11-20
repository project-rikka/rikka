use glam::Vec3;

pub const STUDIO_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StudioLight {
    pub direction: Vec3,
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub wrap: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StudioLighting {
    pub ambient: Vec3,
    pub lights: [StudioLight; STUDIO_LIGHTS],
}
