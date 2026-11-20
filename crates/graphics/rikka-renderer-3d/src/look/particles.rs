use glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParticleKind {
    #[default]
    Motes,
    Petals,
    Sparkles,
    Snow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particles {
    pub amount: f32,
    pub kind: ParticleKind,
    pub size: f32,
    pub color: Vec3,
    pub glow: f32,
    pub wind: Vec3,
    pub reach: f32,
    pub focus: f32,
    pub seed: u32,
}

impl Default for Particles {
    fn default() -> Self {
        Self {
            amount: 0.0,
            kind: ParticleKind::Motes,
            size: 1.0,
            color: Vec3::new(1.0, 0.92, 0.8),
            glow: 1.0,
            wind: Vec3::new(0.3, 0.05, 0.1),
            reach: 8.0,
            focus: 3.0,
            seed: 0,
        }
    }
}
