use rikka_model_core::glam::Vec3;
use rikka_model_core::{Error, HumanBone, ImportCtx, Input, Model, Result, Warning};
use serde::Deserialize;

use super::resolve::Resolved;
use crate::VrmAnimationImporter;
use crate::animation::{AnimatedExpression, AnimatedLookAt, VrmAnimation};
use crate::document as raw;
use crate::expression::{ExpressionName, ExpressionPreset};

const MAJOR: &str = "1.";

const UNANIMATED_BONES: [HumanBone; 2] = [HumanBone::LeftEye, HumanBone::RightEye];

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let rikka_gltf::Parts {
        mut model,
        meshes,
        described: document,
        ..
    } = rikka_gltf::parts(input, &[raw::VRM_ANIMATION], ctx)?;
    let value = document
        .extension(raw::VRM_ANIMATION)
        .ok_or_else(|| Error::Damaged(format!("no \"{}\" extension", raw::VRM_ANIMATION)))?;
    let described = raw::VrmAnimation::deserialize(value)
        .map_err(|error| Error::Damaged(format!("invalid \"{}\": {error}", raw::VRM_ANIMATION)))?;
    if !described.spec_version.starts_with(MAJOR) {
        return Err(Error::UnsupportedVersion {
            format: VrmAnimationImporter::NAME,
            version: described.spec_version,
        });
    }
    if described.spec_version != raw::SPEC_VERSION {
        ctx.warn(Warning::Unsupported(format!(
            "{} {} is read as {}",
            VrmAnimationImporter::NAME,
            described.spec_version,
            raw::SPEC_VERSION
        )));
    }
    let resolved = Resolved::new(&model, &meshes);
    let mut humanoid = described
        .humanoid
        .as_ref()
        .map(|humanoid| {
            super::humanoid(
                humanoid
                    .human_bones
                    .iter()
                    .map(|(name, bone)| (name.as_str(), bone.node)),
                false,
                &resolved,
                ctx,
            )
        })
        .unwrap_or_default();
    for bone in UNANIMATED_BONES {
        if humanoid.bone(bone).is_some() {
            ctx.warn(Warning::Repaired(format!(
                "ignoring {bone}: gaze is animated through lookAt"
            )));
            humanoid.set_bone(bone, None);
        }
    }
    let expressions = described
        .expressions
        .as_ref()
        .map(|expressions| self::expressions(expressions, &resolved, ctx))
        .unwrap_or_default();
    let look_at = described.look_at.as_ref().map(|look_at| AnimatedLookAt {
        node: look_at
            .node
            .and_then(|node| resolved.node(node, "lookAt", ctx)),
        offset_from_head: look_at.offset_from_head_bone.map(Vec3::from),
    });
    model.extensions.insert(VrmAnimation {
        humanoid,
        expressions,
        look_at,
    });
    Ok(model)
}

fn expressions(
    described: &raw::AnimatedExpressions,
    resolved: &Resolved<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Vec<AnimatedExpression> {
    let mut expressions = Vec::new();
    for (name, animated) in &described.preset {
        let Some(preset) = ExpressionPreset::from_name(name) else {
            ctx.warn(Warning::Unsupported(format!(
                "ignoring unknown preset expression \"{name}\""
            )));
            continue;
        };
        if preset.is_look_at() {
            ctx.warn(Warning::Repaired(format!(
                "ignoring expression \"{name}\": gaze is animated through lookAt"
            )));
            continue;
        }
        if let Some(node) = resolved.node(animated.node, &format!("expression {name}"), ctx) {
            expressions.push(AnimatedExpression {
                name: ExpressionName::Preset(preset),
                node,
            });
        }
    }
    for (name, animated) in &described.custom {
        if ExpressionPreset::from_name(name).is_some() {
            ctx.warn(Warning::Repaired(format!(
                "ignoring custom expression \"{name}\": it repeats a preset name"
            )));
            continue;
        }
        if let Some(node) = resolved.node(animated.node, &format!("expression {name}"), ctx) {
            expressions.push(AnimatedExpression {
                name: ExpressionName::Custom(name.clone()),
                node,
            });
        }
    }
    expressions
}
