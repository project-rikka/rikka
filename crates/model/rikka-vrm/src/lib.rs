macro_rules! named {
    ($kind:ident { $($variant:ident => $name:literal),* $(,)? }) => {
        impl $kind {
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),*
                }
            }

            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }

        impl std::fmt::Display for $kind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}

mod animation;
mod avatar;
mod constraint;
mod document;
mod expression;
mod expressive;
mod extended_collider;
mod first_person;
mod legacy;
mod legacy_expression;
mod legacy_humanoid;
mod license;
mod look_at;
mod look_at_curves;
mod meta;
mod read;
mod runtime;
mod spring;
mod vrm_legacy;
mod write;

pub use animation::{AnimatedExpression, AnimatedLookAt, Gaze, VrmAnimation};
pub use avatar::{Vrm, VrmVersion};
pub use constraint::{AimAxis, Constraint, ConstraintState, NodeConstraint, RollAxis};
pub use expression::{
    Expression, ExpressionName, ExpressionOverride, ExpressionPreset, MaterialColor,
    MaterialColorBind, MorphBind, Procedural, TextureTransformBind,
};
pub use extended_collider::{ExtendedCollider, ExtendedColliders, ExtendedShape};
pub use first_person::{FirstPersonKind, MeshAnnotation};
pub use legacy_expression::{LegacyExpression, LegacyMaterialValue};
pub use legacy_humanoid::{BoneLimit, LegacyHumanoid};
pub use license::{LegacyLicense, LegacyUser, LicenseName};
pub use look_at::{LookAt, LookAtKind, RangeMap};
pub use look_at_curves::LookAtCurves;
pub use meta::{AvatarPermission, CommercialUsage, CreditNotation, Meta, Modification};
pub use spring::{Collider, ColliderGroup, ColliderShape, Spring, SpringBone, SpringJoint};
pub use vrm_legacy::VrmLegacy;

pub use expressive::Expressive;
pub use runtime::Avatar;

use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};

const NAME: &str = "VRM";

#[derive(Clone, Copy, Debug, Default)]
pub struct VrmImporter;

impl Importer for VrmImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["vrm"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        rikka_gltf::GltfImporter.sniff(head)
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::read(input, ctx)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VrmAnimationImporter;

impl VrmAnimationImporter {
    pub const NAME: &'static str = "VRM Animation";
}

impl Importer for VrmAnimationImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: Self::NAME,
            extensions: &["vrma"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        rikka_gltf::GltfImporter.sniff(head)
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::animation::read(input, ctx)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VrmExporter;

impl Exporter for VrmExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: NAME,
            extensions: &["vrm"],
            supports: Supports::all(),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VrmLegacyExporter;

impl VrmLegacyExporter {
    pub const NAME: &'static str = "VRM 0.x";
}

impl Exporter for VrmLegacyExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: Self::NAME,
            extensions: &["vrm"],
            supports: Supports::none().skins(true).morph_targets(true),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write_legacy(model, ctx)
    }
}
