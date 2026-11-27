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

use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{Conversion, CoordinateSystem, Extension, ImportOptions, Limit, Limits};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};
use rikka_parallel::Parallel;

use crate::MMD_UNIT_SCALE;

const NAME: &str = "VMD";

pub(crate) const PARALLEL: Parallel = Parallel::new("rikka-mmd");

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
        let motion = read::read(input.bytes(), &|| ctx.checkpoint())?;
        check_span(&motion, &ctx.options().limits)?;
        animation::model_of(motion, ctx)
    }
}

impl Vmd {
    pub fn read(bytes: &[u8], options: &ImportOptions) -> Result<Vmd> {
        let mut motion = read::read(bytes, &|| Ok(()))?;
        check_span(&motion, &options.limits)?;
        let units = match options.normalize_units {
            true => MMD_UNIT_SCALE,
            false => 1.0,
        };
        let conversion = Conversion::new(
            CoordinateSystem::LEFT_HANDED_Y_UP,
            options.coordinates,
            units * options.scale,
        );
        if !conversion.is_identity() {
            motion.convert(&conversion);
        }
        Ok(motion)
    }
}

fn check_span(motion: &Vmd, limits: &Limits) -> Result<()> {
    limits.check(Limit::Keyframes, u64::from(motion.last_frame()) + 1)
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
