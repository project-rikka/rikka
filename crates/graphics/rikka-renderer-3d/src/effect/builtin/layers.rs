use crate::effect::builtin::{BuiltinEffect, effect, image, pass, read, single, target, write};
use crate::effect::desc::{
    Effect, EffectCode, EffectFormat, EffectInput, EffectOutput, EffectStep, EffectUniform,
};

impl Effect {
    pub(crate) fn layer_tint() -> Self {
        single(BuiltinEffect::LayerTint, "tint", |step| {
            step.read("layers", EffectInput::Layers)
                .read("depth", EffectInput::Depth)
        })
    }

    pub(crate) fn outline() -> Self {
        single(BuiltinEffect::Outline, "trace", |step| {
            step.read("depth", EffectInput::Depth)
                .read("normals", EffectInput::Normals)
                .read("layers", EffectInput::Layers)
                .uniform("frame", EffectUniform::Frame)
        })
    }

    pub(crate) fn layer_glow() -> Self {
        let half = || target(EffectFormat::R32Float, 2);
        effect(BuiltinEffect::LayerGlow)
            .target("mask", half())
            .target("spread", half())
            .target("halo", half())
            .step(
                EffectStep::new(EffectCode::builtin("layer_glow_mask", "choose"))
                    .label("choose")
                    .read("layers", EffectInput::Layers)
                    .write("target", write("mask"))
                    .uniform("size", EffectUniform::Size),
            )
            .step(pass(
                "layer_glow_blur",
                "horizontal",
                read("mask"),
                write("spread"),
            ))
            .step(pass(
                "layer_glow_blur",
                "vertical",
                read("spread"),
                write("halo"),
            ))
            .step(
                pass("layer_glow", "combine", image(), EffectOutput::Image)
                    .read("halo", read("halo"))
                    .read("layers", EffectInput::Layers),
            )
    }

    pub(crate) fn layer_bloom() -> Self {
        let extract = pass("layer_bloom_bright", "extract", image(), write("bright"))
            .read("layers", EffectInput::Layers);
        bloom(BuiltinEffect::LayerBloom, extract)
    }

    pub(crate) fn emission_bloom() -> Self {
        let extract = EffectStep::new(EffectCode::builtin("emission_bloom_bright", "extract"))
            .label("extract")
            .read("emission", EffectInput::Emission)
            .write("target", write("bright"))
            .uniform("size", EffectUniform::Size);
        bloom(BuiltinEffect::EmissionBloom, extract)
    }
}

fn bloom(builtin: BuiltinEffect, extract: EffectStep) -> Effect {
    let name = builtin.name();
    let blurring = format!("{name}_blur");
    let blur = |entry: &str, from: &str, to: &str| pass(&blurring, entry, read(from), write(to));
    effect(builtin)
        .target("bright", target(EffectFormat::Rgba16Float, 4))
        .target("near_spread", target(EffectFormat::Rgba16Float, 4))
        .target("near", target(EffectFormat::Rgba16Float, 4))
        .target("wide_source", target(EffectFormat::Rgba16Float, 8))
        .target("wide_spread", target(EffectFormat::Rgba16Float, 8))
        .target("wide", target(EffectFormat::Rgba16Float, 8))
        .step(extract)
        .step(blur("horizontal", "bright", "near_spread"))
        .step(blur("vertical", "near_spread", "near"))
        .step(pass(
            "bloom_reduce",
            "reduce",
            read("near"),
            write("wide_source"),
        ))
        .step(blur("horizontal", "wide_source", "wide_spread"))
        .step(blur("vertical", "wide_spread", "wide"))
        .step(
            pass(name, "combine", image(), EffectOutput::Image)
                .read("near", read("near"))
                .read("wide", read("wide")),
        )
}
