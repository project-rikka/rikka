use glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EmitterShape {
    #[default]
    Point,
    Sphere {
        radius: f32,
    },
    Box {
        half_extents: Vec3,
    },
    Disc {
        radius: f32,
    },
    Cone {
        radius: f32,
        angle: f32,
    },
    Line {
        length: f32,
    },
}

impl EmitterShape {
    pub(crate) fn code(self) -> u32 {
        match self {
            Self::Point => 0,
            Self::Sphere { .. } => 1,
            Self::Box { .. } => 2,
            Self::Disc { .. } => 3,
            Self::Cone { .. } => 4,
            Self::Line { .. } => 5,
        }
    }

    pub(crate) fn radius(self) -> f32 {
        match self {
            Self::Sphere { radius } | Self::Disc { radius } | Self::Cone { radius, .. } => radius,
            Self::Line { length } => length,
            Self::Point | Self::Box { .. } => 0.0,
        }
    }

    pub(crate) fn extent(self) -> Vec3 {
        match self {
            Self::Box { half_extents } => half_extents,
            _ => Vec3::ZERO,
        }
    }

    pub(crate) fn angle(self) -> f32 {
        match self {
            Self::Cone { angle, .. } => angle,
            _ => 0.0,
        }
    }

    pub(crate) fn sample(self, s: [f32; 3], d: [f32; 2]) -> (Vec3, Vec3) {
        let code = self.code();
        let radius = self.radius();
        let extent = self.extent();
        let angle = self.angle();
        local_sample(code, radius, extent, angle, s, d)
    }
}

fn local_sample(
    code: u32,
    radius: f32,
    extent: Vec3,
    angle: f32,
    s: [f32; 3],
    d: [f32; 2],
) -> (Vec3, Vec3) {
    let tau = std::f32::consts::TAU;
    let up = Vec3::Y;
    let disc = || {
        let turn = tau * s[0];
        let reach = radius * s[1].sqrt();
        Vec3::new(reach * turn.cos(), 0.0, reach * turn.sin())
    };
    match code {
        0 => (Vec3::ZERO, sphere(d)),
        1 => {
            let direction = sphere(d);
            (direction * (radius * s[0].powf(1.0 / 3.0)), direction)
        }
        2 => (
            Vec3::new(
                (2.0 * s[0] - 1.0) * extent.x,
                (2.0 * s[1] - 1.0) * extent.y,
                (2.0 * s[2] - 1.0) * extent.z,
            ),
            up,
        ),
        3 => (disc(), up),
        4 => {
            let cos_theta = 1.0 - d[0] * (1.0 - angle.cos());
            let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
            let phi = tau * d[1];
            (
                disc(),
                Vec3::new(sin_theta * phi.cos(), cos_theta, sin_theta * phi.sin()),
            )
        }
        _ => (Vec3::new((s[0] - 0.5) * radius, 0.0, 0.0), up),
    }
}

fn sphere(d: [f32; 2]) -> Vec3 {
    let y = 1.0 - 2.0 * d[0];
    let reach = (1.0 - y * y).max(0.0).sqrt();
    let phi = std::f32::consts::TAU * d[1];
    Vec3::new(reach * phi.cos(), y, reach * phi.sin())
}
