use super::access::{JointType, Params, ParamsMut, Sealed};
use super::axes::Axes;
use super::builder::{Joint, JointDef};
use super::d6::D6;
use super::kinds::JointParams;
use super::motor::Motor;
use super::parallel::Parallel;
use super::prismatic::Prismatic;
use super::revolute::Revolute;
use super::wheel::Wheel;

macro_rules! axes_joints {
    ($($variant:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub(crate) enum AxesParams {
            $($variant($variant)),*
        }

        impl AxesParams {
            pub(crate) fn axes(&self) -> &Axes {
                match self {
                    $(AxesParams::$variant(joint) => joint.axes()),*
                }
            }

            pub(crate) fn is_valid(&self) -> bool {
                self.axes().is_valid()
            }
        }

        $(
            impl From<Joint<$variant>> for JointDef {
                fn from(joint: Joint<$variant>) -> Self {
                    Self {
                        base: joint.base,
                        params: JointParams::Axes(AxesParams::$variant(joint.params)),
                    }
                }
            }

            impl Sealed for $variant {
                fn from_params(params: Params<'_>) -> Option<&Self> {
                    match params.0 {
                        JointParams::Axes(AxesParams::$variant(joint)) => Some(joint),
                        _ => None,
                    }
                }

                fn from_params_mut(params: ParamsMut<'_>) -> Option<&mut Self> {
                    match params.0 {
                        JointParams::Axes(AxesParams::$variant(joint)) => Some(joint),
                        _ => None,
                    }
                }
            }

            impl JointType for $variant {}
        )*
    };
}

axes_joints!(D6, Revolute, Prismatic, Wheel, Motor, Parallel);
