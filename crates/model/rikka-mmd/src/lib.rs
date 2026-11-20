mod euler;
pub(crate) mod pmx;
mod runtime;
mod text;
pub(crate) mod vmd;

use std::borrow::Cow;

use rikka_model_core::CoordinateSystem;
use rikka_model_core::Model;
use rikka_model_core::convert_coordinates;

pub use pmx::{
    Pmx, Pmx21Exporter, PmxAeroModel, PmxAnchor, PmxAppend, PmxBlend, PmxBone, PmxBoneFlags,
    PmxBoneOffset, PmxClusterConfig, PmxDisplayFrame, PmxDisplayTarget, PmxExporter,
    PmxGroupOffset, PmxIk, PmxIkLink, PmxImporter, PmxImpulseOffset, PmxIndexSizes, PmxJoint,
    PmxJointKind, PmxLimits, PmxMaterial, PmxMaterialFlags, PmxMaterialOffset, PmxMorph,
    PmxOffsets, PmxPanel, PmxPhysics, PmxRigidBody, PmxShape, PmxSoftBody, PmxSoftBodyConfig,
    PmxSoftBodyFlags, PmxSoftBodyIterations, PmxSoftBodyMaterial, PmxSoftBodyShape, PmxSphereMode,
    PmxSpherical, PmxTail, PmxToon, PmxUvOffset, PmxVersion, PmxVertices, PmxWeight,
    standard_skeleton,
};
pub use text::PmxEncoding;
pub use vmd::{
    CAMERA_MOTION, FRAMES_PER_SECOND, Vmd, VmdBoneFrame, VmdBones, VmdCamera, VmdCameraFrame,
    VmdCameraInterpolation, VmdCameraPose, VmdCurve, VmdExporter, VmdImporter, VmdInterpolation,
    VmdLightFrame, VmdMorphFrame, VmdPropertyFrame, VmdShadowFrame, VmdSwitches,
};

pub use runtime::Puppet;

pub(crate) const MAGIC: &[u8] = b"PMX ";

pub const MMD_UNIT_SCALE: f32 = 0.08;

pub(crate) const EDGE_SCREEN_WIDTH: f32 = 0.001;

pub(crate) fn in_mmd_space(model: &Model) -> Cow<'_, Model> {
    let is_mmd_handed = model.metadata.coordinates == CoordinateSystem::LEFT_HANDED_Y_UP;
    let scale = model.metadata.unit_scale / MMD_UNIT_SCALE;
    if is_mmd_handed && (scale - 1.0).abs() < 1e-6 {
        return Cow::Borrowed(model);
    }
    let mut copy = model.clone();
    convert_coordinates(&mut copy, CoordinateSystem::LEFT_HANDED_Y_UP, scale);
    copy.metadata.unit_scale = MMD_UNIT_SCALE;
    Cow::Owned(copy)
}

pub(crate) fn model_name(model: &Model) -> String {
    model
        .roots
        .first()
        .map(|root| model.node(*root).name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "model".to_owned())
}
