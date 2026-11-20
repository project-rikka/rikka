mod ascii;
pub(crate) mod binary;
mod build;

use rikka_model_core::{Confidence, ImportCtx, Input, Model, Result, Stage, Warning};

const SOLID: &[u8] = b"solid";

pub(crate) fn sniff(head: &[u8]) -> Confidence {
    if opens_solid(head) {
        let text = String::from_utf8_lossy(head).to_ascii_lowercase();
        return match text.contains("facet") || text.contains("endsolid") {
            true => Confidence::Yes,
            false => Confidence::Maybe,
        };
    }
    match binary::plausible(head) {
        true => Confidence::Maybe,
        false => Confidence::No,
    }
}

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let bytes = input.bytes();
    let document = match layout(bytes) {
        Layout::Binary { trailing } => {
            if trailing > 0 {
                ctx.warn(Warning::Repaired(format!(
                    "ignoring {trailing} trailing bytes after last triangle"
                )));
            }
            binary::parse(bytes, ctx)?
        }
        Layout::Text => ascii::parse(bytes, ctx)?,
    };
    ctx.report(Stage::Parsing, 0.8);
    let name = input
        .path()
        .and_then(|path| path.file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    build::build(document, &name, binary::generator(bytes))
}

enum Layout {
    Binary { trailing: usize },
    Text,
}

fn layout(bytes: &[u8]) -> Layout {
    let declared = binary::declared_length(bytes);
    if declared == Some(bytes.len()) {
        return Layout::Binary { trailing: 0 };
    }
    if opens_solid(bytes) && !bytes[..bytes.len().min(512)].contains(&0) {
        return Layout::Text;
    }
    match declared {
        Some(length) if length < bytes.len() => Layout::Binary {
            trailing: bytes.len() - length,
        },
        _ => match opens_solid(bytes) {
            true => Layout::Text,
            false => Layout::Binary { trailing: 0 },
        },
    }
}

fn opens_solid(bytes: &[u8]) -> bool {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let rest = &bytes[start..];
    rest.len() >= SOLID.len()
        && rest[..SOLID.len()].eq_ignore_ascii_case(SOLID)
        && rest
            .get(SOLID.len())
            .is_none_or(|byte| byte.is_ascii_whitespace())
}
