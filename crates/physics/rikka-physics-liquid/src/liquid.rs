#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Liquid {
    pub viscosity: f32,
    pub elasticity: f32,
    pub relaxation: f32,
    pub cohesion: f32,
    pub adhesion: f32,
    pub friction: f32,
    pub spray: f32,
    pub wetting: f32,
    pub color: [f32; 3],
    pub depth: f32,
    pub cloudiness: f32,
    pub roughness: f32,
    pub ior: f32,
    pub tint: [f32; 3],
}
