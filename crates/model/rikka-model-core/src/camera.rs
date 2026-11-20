use crate::id::model_id;

model_id!(CameraId);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    Perspective {
        y_fov: f32,
        aspect: Option<f32>,
        near: f32,
        far: Option<f32>,
    },
    Orthographic {
        width: f32,
        height: f32,
        near: f32,
        far: f32,
    },
}

impl Projection {
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        match self {
            Self::Perspective {
                y_fov,
                aspect,
                near,
                far,
            } => Self::Perspective {
                y_fov,
                aspect,
                near: near * factor,
                far: far.map(|far| far * factor),
            },
            Self::Orthographic {
                width,
                height,
                near,
                far,
            } => Self::Orthographic {
                width: width * factor,
                height: height * factor,
                near: near * factor,
                far: far * factor,
            },
        }
    }
}

impl Default for Projection {
    fn default() -> Self {
        Self::Perspective {
            y_fov: std::f32::consts::FRAC_PI_3,
            aspect: None,
            near: 0.05,
            far: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Camera {
    pub name: String,
    pub projection: Projection,
}

impl Camera {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn projection(mut self, projection: Projection) -> Self {
        self.projection = projection;
        self
    }
}
