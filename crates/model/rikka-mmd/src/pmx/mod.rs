mod document;
mod read;
mod soft_body;
mod standard;
pub(crate) mod surface;
mod version;
mod write;

pub use document::{
    Pmx, PmxAppend, PmxBlend, PmxBone, PmxBoneFlags, PmxBoneOffset, PmxDisplayFrame,
    PmxDisplayTarget, PmxGroupOffset, PmxIk, PmxIkLink, PmxImpulseOffset, PmxIndexSizes, PmxJoint,
    PmxJointKind, PmxLimits, PmxMaterial, PmxMaterialFlags, PmxMaterialOffset, PmxMorph,
    PmxOffsets, PmxPanel, PmxPhysics, PmxRigidBody, PmxShape, PmxSphereMode, PmxSpherical, PmxTail,
    PmxToon, PmxUvOffset, PmxVertices, PmxWeight,
};
pub use soft_body::{
    PmxAeroModel, PmxAnchor, PmxClusterConfig, PmxSoftBody, PmxSoftBodyConfig, PmxSoftBodyFlags,
    PmxSoftBodyIterations, PmxSoftBodyMaterial, PmxSoftBodyShape,
};
pub use standard::standard_skeleton;
pub use version::PmxVersion;

use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Confidence, ImportCtx, Importer, ImporterInfo, Input};
use rikka_model_core::{ExportCtx, Exporter, ExporterInfo, Supports};

const NAME: &str = "PMX";
const NAME_2_1: &str = "PMX 2.1";

#[derive(Clone, Copy, Debug, Default)]
pub struct PmxImporter;

impl Importer for PmxImporter {
    fn info(&self) -> ImporterInfo {
        ImporterInfo {
            name: NAME,
            extensions: &["pmx"],
        }
    }

    fn sniff(&self, head: &[u8]) -> Confidence {
        match read::sniffs(head) {
            true => Confidence::Yes,
            false => Confidence::No,
        }
    }

    fn read(&self, input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
        read::read(input, ctx)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PmxExporter;

impl Exporter for PmxExporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: NAME,
            extensions: &["pmx"],
            supports: Supports::none().skins(true).morph_targets(true),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx, PmxVersion::V2_0)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Pmx21Exporter;

impl Exporter for Pmx21Exporter {
    fn info(&self) -> ExporterInfo {
        ExporterInfo {
            name: NAME_2_1,
            extensions: &["pmx"],
            supports: Supports::none().skins(true).morph_targets(true),
        }
    }

    fn write(&self, model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
        write::write(model, ctx, PmxVersion::V2_1)
    }
}
