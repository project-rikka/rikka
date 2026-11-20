use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cavity {
    pub ridge: f32,
    pub valley: f32,
    pub distance: f32,
    pub attenuation: f32,
    pub samples: u32,
}

impl Default for Cavity {
    fn default() -> Self {
        Self {
            ridge: 1.0,
            valley: 1.0,
            distance: 0.2,
            attenuation: 1.0,
            samples: 16,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curvature {
    pub ridge: f32,
    pub valley: f32,
}

impl Default for Curvature {
    fn default() -> Self {
        Self {
            ridge: 1.0,
            valley: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioShadow {
    pub direction: Vec3,
    pub intensity: f32,
    pub shift: f32,
    pub focus: f32,
}

impl Default for StudioShadow {
    fn default() -> Self {
        Self {
            direction: Vec3::splat(-1.0).normalize(),
            intensity: 0.5,
            shift: 0.1,
            focus: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkbenchEffects {
    pub cavity: Option<Cavity>,
    pub curvature: Option<Curvature>,
    pub outline: Option<Vec3>,
    pub shadow: Option<StudioShadow>,
    pub pixel_size: f32,
}

impl WorkbenchEffects {
    pub(crate) fn composites(&self) -> bool {
        self.cavity.is_some() || self.curvature.is_some() || self.outline.is_some()
    }
}
