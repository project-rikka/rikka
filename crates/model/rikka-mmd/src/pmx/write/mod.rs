mod build;
mod soft_body;
mod validate;
mod wire;

use rikka_model_core::{ByteWriter, ExportCtx, Model, Result, Stage, Warning};

use crate::pmx::document::{Pmx, PmxIndexSizes};
use crate::pmx::version::PmxVersion;
use crate::text::PmxEncoding;
use crate::{MAGIC, in_mmd_space, model_name};

use build::{
    PmxGeometry, additional_uvs, bones_of, frames_of, materials_of, morphs_of, write_textures,
};
use soft_body::write_soft_bodies;
use wire::{
    write_bones, write_faces, write_frames, write_joints, write_materials, write_morphs,
    write_rigid_bodies, write_vertices,
};

pub(crate) fn write(
    model: &Model,
    ctx: &mut ExportCtx<'_>,
    minimum: PmxVersion,
) -> Result<Vec<u8>> {
    let model = in_mmd_space(model);
    let geometry = PmxGeometry::gather(&model)?;
    ctx.report(Stage::Writing, 0.2);

    let pmx = model.extensions.get::<Pmx>();
    let source_pmx = pmx.filter(|source| geometry.matches(source));
    if pmx.is_some() && source_pmx.is_none() {
        ctx.warn(Warning::Repaired(
            "model no longer matches its PMX extension, dropping PMX-only data".to_owned(),
        ));
    }
    if let Some(source) = source_pmx {
        validate::validate(source, geometry.positions.len())?;
    }
    let version = source_pmx.map_or(minimum, |source| minimum.max(source.required_version()));

    let textures = write_textures(&model, source_pmx, ctx)?;
    ctx.report(Stage::Resources, 1.0);
    let materials = materials_of(&model, &geometry, source_pmx, &textures);
    let bones = bones_of(&geometry, source_pmx);
    let morphs = morphs_of(&geometry, source_pmx);
    let frames = frames_of(source_pmx, &bones, &morphs);

    let encoding = source_pmx.map_or(PmxEncoding::Utf16, |source| source.encoding);
    let sizes = PmxIndexSizes::fitting(
        geometry.positions.len(),
        textures.len(),
        materials.len(),
        bones.len(),
        morphs.len(),
        source_pmx.map_or(0, |source| source.rigid_bodies.len()),
    );
    let empty = Pmx::default();
    let names = source_pmx.unwrap_or(&empty);

    let mut writer = ByteWriter::with_capacity(geometry.positions.len() * 64);
    writer.bytes(MAGIC);
    writer.f32(version.number());
    writer.u8(8);
    writer.bytes(&[
        encoding.byte(),
        additional_uvs(source_pmx),
        sizes.vertex,
        sizes.texture,
        sizes.material,
        sizes.bone,
        sizes.morph,
        sizes.rigid_body,
    ]);
    let model_name = match names.name.is_empty() {
        true => model_name(&model),
        false => names.name.clone(),
    };
    encoding.write(&mut writer, &model_name);
    encoding.write(&mut writer, &names.english_name);
    encoding.write(&mut writer, &names.comment);
    encoding.write(&mut writer, &names.english_comment);

    write_vertices(&mut writer, &geometry, source_pmx, &sizes, ctx)?;
    write_faces(&mut writer, &geometry, &sizes);
    writer.i32(textures.len() as i32);
    for path in &textures {
        encoding.write(&mut writer, path);
    }
    write_materials(&mut writer, &materials, &geometry, encoding, &sizes);
    ctx.report(Stage::Writing, 0.7);
    write_bones(&mut writer, &bones, encoding, &sizes);
    write_morphs(&mut writer, &morphs, &geometry, encoding, &sizes);
    write_frames(&mut writer, &frames, encoding, &sizes);
    write_rigid_bodies(&mut writer, &names.rigid_bodies, encoding, &sizes);
    write_joints(&mut writer, &names.joints, encoding, &sizes);
    if version == PmxVersion::V2_1 {
        write_soft_bodies(&mut writer, &names.soft_bodies, encoding, &sizes);
    }
    ctx.report(Stage::Writing, 1.0);
    Ok(writer.finish())
}
