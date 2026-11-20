use crate::effect::builtin::{BuiltinEffect, effect, image, pass, read, single, target, write};
use crate::effect::desc::{Effect, EffectFormat, EffectInput, EffectOutput, EffectUniform};

impl Effect {
    pub(crate) fn dilate_erode() -> Self {
        let full = || target(EffectFormat::Rgba16Float, 1);
        let morph = |entry: &str, source, output| {
            pass("dilate_erode", entry, source, output).read("image", image())
        };
        effect(BuiltinEffect::DilateErode)
            .target("wide", full())
            .target("square", full())
            .target("diamond", full())
            .step(morph("horizontal", image(), write("wide")))
            .step(morph("vertical", read("wide"), write("square")))
            .step(morph("diagonal", read("square"), write("diamond")))
            .step(morph("antidiagonal", read("diamond"), EffectOutput::Image))
    }

    pub(crate) fn median() -> Self {
        single(BuiltinEffect::Median, "despeckle", |step| step)
    }

    pub(crate) fn unsharp_mask() -> Self {
        let full = || target(EffectFormat::Rgba16Float, 1);
        effect(BuiltinEffect::UnsharpMask)
            .target("spread", full())
            .target("blurred", full())
            .step(pass(
                "unsharp_mask_blur",
                "horizontal",
                image(),
                write("spread"),
            ))
            .step(pass(
                "unsharp_mask_blur",
                "vertical",
                read("spread"),
                write("blurred"),
            ))
            .step(
                pass("unsharp_mask", "sharpen", image(), EffectOutput::Image)
                    .read("blurred", read("blurred")),
            )
    }

    pub(crate) fn bilateral() -> Self {
        let guided = |entry: &str, source, output| {
            pass("bilateral", entry, source, output)
                .read("image", image())
                .read("depth", EffectInput::Depth)
                .read("normals", EffectInput::Normals)
                .uniform("frame", EffectUniform::Frame)
        };
        effect(BuiltinEffect::Bilateral)
            .target("spread", target(EffectFormat::Rgba16Float, 1))
            .step(guided("horizontal", image(), write("spread")))
            .step(guided("vertical", read("spread"), EffectOutput::Image))
    }

    pub(crate) fn edge_detect() -> Self {
        single(BuiltinEffect::EdgeDetect, "detect", |step| {
            step.read("depth", EffectInput::Depth)
                .read("normals", EffectInput::Normals)
                .uniform("frame", EffectUniform::Frame)
        })
    }
}
