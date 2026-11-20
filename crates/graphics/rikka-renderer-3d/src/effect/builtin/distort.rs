use crate::effect::builtin::{BuiltinEffect, effect, image, pass, read, single, target, write};
use crate::effect::desc::{Effect, EffectFormat, EffectInput, EffectOutput, EffectUniform};

impl Effect {
    pub(crate) fn panini() -> Self {
        single(BuiltinEffect::Panini, "project", |step| {
            step.uniform("frame", EffectUniform::Frame)
        })
    }

    pub(crate) fn lens_distortion() -> Self {
        single(BuiltinEffect::LensDistortion, "warp", |step| step)
    }

    pub(crate) fn anamorphic() -> Self {
        single(BuiltinEffect::Anamorphic, "squeeze", |step| step)
    }

    pub(crate) fn radial_blur() -> Self {
        effect(BuiltinEffect::RadialBlur)
            .target("fine", target(EffectFormat::Rgba16Float, 1))
            .step(pass("radial_blur", "fine", image(), write("fine")))
            .step(pass(
                "radial_blur",
                "coarse",
                read("fine"),
                EffectOutput::Image,
            ))
    }

    pub(crate) fn heat_distortion() -> Self {
        single(BuiltinEffect::HeatDistortion, "shimmer", |step| {
            step.read("depth", EffectInput::Depth)
                .read("layers", EffectInput::Layers)
                .uniform("frame", EffectUniform::Frame)
                .uniform("time", EffectUniform::Time)
        })
    }
}
