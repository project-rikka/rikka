mod build;
mod soft_body;
mod wire;

pub(super) use build::attach_bones;

use rikka_model_core::{ImportCtx, Input, Model, Result};

use crate::MAGIC;

pub(crate) fn sniffs(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let parsed = wire::parse(input.bytes(), ctx)?;
    build::build(parsed, ctx)
}
