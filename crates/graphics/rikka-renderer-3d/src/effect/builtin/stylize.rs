use crate::effect::builtin::{BuiltinEffect, effect, image, pass, read, single, target, write};
use crate::effect::desc::{Effect, EffectFormat, EffectInput, EffectOutput, EffectUniform};

impl Effect {
    pub(crate) fn kuwahara() -> Self {
        let full = || target(EffectFormat::Rgba16Float, 1);
        effect(BuiltinEffect::Kuwahara)
            .target("tensor", full())
            .target("spread", full())
            .target("smooth", full())
            .step(pass(
                "kuwahara_tensor",
                "structure",
                image(),
                write("tensor"),
            ))
            .step(pass(
                "kuwahara_blur",
                "horizontal",
                read("tensor"),
                write("spread"),
            ))
            .step(pass(
                "kuwahara_blur",
                "vertical",
                read("spread"),
                write("smooth"),
            ))
            .step(
                pass("kuwahara", "filter", image(), EffectOutput::Image)
                    .read("tensor", read("smooth")),
            )
    }

    pub(crate) fn halftone() -> Self {
        single(BuiltinEffect::Halftone, "screen", |step| step)
    }

    pub(crate) fn screentone() -> Self {
        single(BuiltinEffect::Screentone, "tone", |step| step)
    }

    pub(crate) fn sketch() -> Self {
        single(BuiltinEffect::Sketch, "hatch", |step| {
            step.read("depth", EffectInput::Depth)
                .uniform("frame", EffectUniform::Frame)
                .uniform("time", EffectUniform::Time)
        })
    }

    pub(crate) fn cel_shade() -> Self {
        single(BuiltinEffect::CelShade, "band", |step| {
            step.read("layers", EffectInput::Layers)
        })
    }

    pub(crate) fn speed_lines() -> Self {
        single(BuiltinEffect::SpeedLines, "streak", |step| {
            step.uniform("time", EffectUniform::Time)
        })
    }

    pub(crate) fn pixelate() -> Self {
        single(BuiltinEffect::Pixelate, "cells", |step| step)
    }

    pub(crate) fn crt() -> Self {
        single(BuiltinEffect::Crt, "scan", |step| step)
    }

    pub(crate) fn sharpen() -> Self {
        single(BuiltinEffect::Sharpen, "sharpen", |step| step)
    }
}
