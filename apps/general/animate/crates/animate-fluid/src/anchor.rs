use rikka_model_core::glam::{Mat4, Vec3};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Anchor {
    #[default]
    World,
    Bone {
        figure: String,
        bone: String,
    },
}

impl Anchor {
    #[must_use]
    pub fn bone(figure: impl Into<String>, bone: impl Into<String>) -> Self {
        Self::Bone {
            figure: figure.into(),
            bone: bone.into(),
        }
    }

    #[must_use]
    pub fn frame(&self, locate: impl FnOnce(&str, &str) -> Option<Mat4>) -> Option<Mat4> {
        match self {
            Self::World => Some(Mat4::IDENTITY),
            Self::Bone { figure, bone } => locate(figure, bone),
        }
    }

    #[must_use]
    pub fn world(
        &self,
        offset: Vec3,
        locate: impl FnOnce(&str, &str) -> Option<Mat4>,
    ) -> Option<Vec3> {
        self.frame(locate)
            .map(|frame| frame.transform_point3(offset))
    }

    #[must_use]
    pub fn offset(
        &self,
        world: Vec3,
        locate: impl FnOnce(&str, &str) -> Option<Mat4>,
    ) -> Option<Vec3> {
        let frame = self.frame(locate)?;
        (frame.determinant().abs() > f32::EPSILON).then(|| frame.inverse().transform_point3(world))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spot {
    pub anchor: Anchor,
    pub offset: Vec3,
}

impl Spot {
    #[must_use]
    pub fn world(&self, locate: impl FnOnce(&str, &str) -> Option<Mat4>) -> Option<Vec3> {
        self.anchor.world(self.offset, locate)
    }
}
