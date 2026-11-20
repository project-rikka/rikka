mod animation;
mod bones;
mod camera;
mod document;
mod framed;
mod read;
mod switches;
mod write;

pub use bones::VmdBones;
pub use camera::{VmdCamera, VmdCameraPose};
pub use document::{
    CAMERA_MOTION, FRAMES_PER_SECOND, Vmd, VmdBoneFrame, VmdCameraFrame, VmdCameraInterpolation,
    VmdCurve, VmdInterpolation, VmdLightFrame, VmdMorphFrame, VmdPropertyFrame, VmdShadowFrame,
};
pub use switches::VmdSwitches;

use rikka_model_core::Limit;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};

const NAME: &str = "VMD";

#[derive(Clone, Copy, Debug, Default)]
pub struct VmdImporter;

impl Importer for VmdImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["vmd"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        match read::sniffs(head) {
            true => Confidence::Yes,
            false => Confidence::No,
        }
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        let motion = read::read(input.bytes(), ctx)?;
        let limits = &ctx.options().limits;
        limits.check(Limit::Keyframes, u64::from(motion.last_frame()) + 1)?;
        let sampled = animation::sampled_keyframes(&motion);
        ctx.charge_keyframes(usize::try_from(sampled).unwrap_or(usize::MAX))?;
        animation::model_of(motion)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VmdExporter;

impl Exporter for VmdExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: NAME,
            extensions: &["vmd"],
            supports: Supports::none()
                .animations(true)
                .morph_targets(true)
                .cameras(true)
                .lights(true),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx)
    }
}
