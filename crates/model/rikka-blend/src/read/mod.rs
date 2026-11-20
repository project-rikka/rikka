pub(crate) mod animation;
pub(crate) mod armature;
mod assemble;
pub(crate) mod camera;
pub(crate) mod idprop;
pub(crate) mod library;
pub(crate) mod light;
pub(crate) mod material;
pub(crate) mod mesh;
pub(crate) mod modifier;
mod motion;
pub(crate) mod resources;
pub(crate) mod scene;
pub(crate) mod transform;

use std::path::Path;

use rikka_model_core::{ImportCtx, Input, Model, Result, Stage};

use crate::file::{Blend, Source};

pub(crate) fn file_name(input: &Input<'_>) -> String {
    input
        .path()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let limit = ctx.options().limits.max_file_bytes;
    ctx.report(Stage::Reading, 0.0);
    let main = Blend::open(Source::Borrowed(input.bytes()), limit)?.with_uri(file_name(input));
    ctx.report(Stage::Resources, 0.1);
    let linked = library::gather(&main, ctx)?;
    let mut blends = vec![main];
    blends.extend(linked);
    let world: &[Blend<'_>] = &blends;
    for blend in world {
        blend.link(world);
    }
    ctx.report(Stage::Parsing, 0.2);
    assemble::assemble(&world[0], ctx)
}
