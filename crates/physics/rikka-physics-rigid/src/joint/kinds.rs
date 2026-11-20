use super::access::{JointType, Params, ParamsMut, Sealed};
use super::axes_model::AxesModel;
use super::axes_params::AxesParams;
use super::builder::{Joint, JointDef};
use super::distance::{Distance, DistanceModel};
use super::filter::{FilterJoint, FilterModel};
use super::model::JointModel;
use super::spherical::{Spherical, SphericalModel};
use super::weld::{Weld, WeldModel};

pub(crate) trait JointVisitor {
    type Output;

    fn visit<M: JointModel>(self) -> Self::Output;
}

macro_rules! joint_kinds {
    ($($variant:ident($params:ty) => $model:ty),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub(crate) enum JointParams {
            $($variant($params)),*
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum JointKind {
            $($variant),*
        }

        impl JointKind {
            pub(crate) const ALL: &[JointKind] = &[$(JointKind::$variant),*];

            #[inline(always)]
            pub(crate) fn dispatch<V: JointVisitor>(self, visitor: V) -> V::Output {
                match self {
                    $(JointKind::$variant => visitor.visit::<$model>()),*
                }
            }
        }

        impl JointParams {
            pub(crate) fn kind(&self) -> JointKind {
                match self {
                    $(JointParams::$variant(_) => JointKind::$variant),*
                }
            }

            pub(crate) fn layout(&self) -> u32 {
                match self {
                    $(JointParams::$variant(params) => <$model as JointModel>::layout(params)),*
                }
            }

            pub(crate) fn is_valid(&self) -> bool {
                match self {
                    $(JointParams::$variant(params) => params.is_valid()),*
                }
            }
        }

        $(
            impl From<Joint<$params>> for JointDef {
                fn from(joint: Joint<$params>) -> Self {
                    Self {
                        base: joint.base,
                        params: JointParams::$variant(joint.params),
                    }
                }
            }

            impl Sealed for $params {
                fn from_params(params: Params<'_>) -> Option<&Self> {
                    match params.0 {
                        JointParams::$variant(params) => Some(params),
                        _ => None,
                    }
                }

                fn from_params_mut(params: ParamsMut<'_>) -> Option<&mut Self> {
                    match params.0 {
                        JointParams::$variant(params) => Some(params),
                        _ => None,
                    }
                }
            }

            impl JointType for $params {}
        )*
    };
}

joint_kinds! {
    Spherical(Spherical) => SphericalModel,
    Weld(Weld) => WeldModel,
    Axes(AxesParams) => AxesModel,
    Distance(Distance) => DistanceModel,
    Filter(FilterJoint) => FilterModel,
}

impl JointKind {
    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) const COUNT: usize = Self::ALL.len();
}
