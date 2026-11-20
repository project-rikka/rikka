use rikka_model_core::glam::{Quat, Vec3};

use crate::euler;
use crate::read::props::Props;
use rikka_model_core::EulerOrder;
use rikka_model_core::Transform;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Inherit {
    #[default]
    Everything,
    RotationOnly,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Pipeline {
    pub order: EulerOrder,
    rotation_space: bool,
    pre_rotation: Vec3,
    post_rotation: Vec3,
    rotation_offset: Vec3,
    rotation_pivot: Vec3,
    scaling_offset: Vec3,
    scaling_pivot: Vec3,
    pub inherit: Inherit,
}

impl Pipeline {
    pub(crate) fn read(props: &Props) -> Self {
        let active = props.integer("RotationActive").unwrap_or(1) != 0;
        let limits_only = props.integer("RotationSpaceForLimitOnly").unwrap_or(0) != 0;
        Self {
            order: euler::order_of(props.integer("RotationOrder").unwrap_or(0)),
            rotation_space: active && !limits_only,
            pre_rotation: props.vec3("PreRotation", Vec3::ZERO),
            post_rotation: props.vec3("PostRotation", Vec3::ZERO),
            rotation_offset: props.vec3("RotationOffset", Vec3::ZERO),
            rotation_pivot: props.vec3("RotationPivot", Vec3::ZERO),
            scaling_offset: props.vec3("ScalingOffset", Vec3::ZERO),
            scaling_pivot: props.vec3("ScalingPivot", Vec3::ZERO),
            inherit: match props.integer("InheritType") {
                Some(2) => Inherit::RotationOnly,
                _ => Inherit::Everything,
            },
        }
    }

    pub(crate) fn local(&self, props: &Props) -> Transform {
        self.place(
            props.vec3("Lcl Translation", Vec3::ZERO),
            props.vec3("Lcl Rotation", Vec3::ZERO),
            props.vec3("Lcl Scaling", Vec3::ONE),
        )
    }

    pub(crate) fn place(&self, translation: Vec3, rotation: Vec3, scaling: Vec3) -> Transform {
        let mut transform = Transform::IDENTITY;
        transform.translation -= self.scaling_pivot;
        transform.scale *= scaling;
        transform.translation *= scaling;
        transform.translation += self.scaling_pivot + self.scaling_offset - self.rotation_pivot;
        let turn = self.rotation(rotation);
        transform.rotation = turn;
        transform.translation = turn * transform.translation;
        transform.translation += self.rotation_pivot + self.rotation_offset + translation;
        transform
    }

    fn rotation(&self, rotation: Vec3) -> Quat {
        match self.rotation_space {
            true => {
                euler::quaternion(EulerOrder::Xyz, self.pre_rotation)
                    * euler::quaternion(self.order, rotation)
                    * euler::quaternion(EulerOrder::Xyz, self.post_rotation).conjugate()
            }
            false => euler::quaternion(EulerOrder::Xyz, rotation),
        }
    }
}

pub(crate) fn geometry(props: &Props) -> Transform {
    Transform {
        translation: props.vec3("GeometricTranslation", Vec3::ZERO),
        rotation: euler::quaternion(EulerOrder::Xyz, props.vec3("GeometricRotation", Vec3::ZERO)),
        scale: props.vec3("GeometricScaling", Vec3::ONE),
    }
}
