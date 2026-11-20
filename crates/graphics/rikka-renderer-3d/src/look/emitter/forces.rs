use glam::Vec3;

pub const STANDARD_GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub speed: f32,
    pub speed_variance: f32,
    pub velocity: Vec3,
    pub inherit: f32,
}

impl Default for Launch {
    fn default() -> Self {
        Self {
            speed: 1.0,
            speed_variance: 0.0,
            velocity: Vec3::ZERO,
            inherit: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turbulence {
    pub strength: f32,
    pub frequency: f32,
    pub flow: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attractor {
    pub position: Vec3,
    pub strength: f32,
    pub rate: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Forces {
    pub gravity: Vec3,
    pub drag: f32,
    pub drag_variance: f32,
    pub wind: Vec3,
    pub turbulence: Turbulence,
    pub attractor: Option<Attractor>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GroundContact {
    #[default]
    Bounce,
    Die,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub height: f32,
    pub contact: GroundContact,
    pub restitution: f32,
    pub friction: f32,
    pub fade: f32,
}

impl Default for Ground {
    fn default() -> Self {
        Self {
            height: 0.0,
            contact: GroundContact::Bounce,
            restitution: 0.4,
            friction: 0.3,
            fade: 0.0,
        }
    }
}

impl Ground {
    pub fn die_at(height: f32) -> Self {
        Self {
            height,
            contact: GroundContact::Die,
            ..Self::default()
        }
    }

    pub fn bounce_at(height: f32, restitution: f32, friction: f32) -> Self {
        Self {
            height,
            contact: GroundContact::Bounce,
            restitution,
            friction,
            fade: 0.0,
        }
    }

    pub fn settle_at(height: f32, fade: f32) -> Self {
        Self {
            height,
            contact: GroundContact::Bounce,
            restitution: 0.0,
            friction: 1.0,
            fade,
        }
    }
}
