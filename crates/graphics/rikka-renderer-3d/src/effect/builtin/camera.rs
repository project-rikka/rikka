use crate::effect::builtin::{BuiltinEffect, effect, image, pass, read, single, target, write};
use crate::effect::desc::{
    Effect, EffectCode, EffectFormat, EffectOutput, EffectStep, EffectUniform,
};

const SINGLE_TEXEL: u32 = 1 << 16;

impl Effect {
    pub(crate) fn vignette() -> Self {
        single(BuiltinEffect::Vignette, "darken", |step| step)
    }

    pub(crate) fn film_grain() -> Self {
        effect(BuiltinEffect::FilmGrain)
            .target("grain", target(EffectFormat::Rgba16Float, 1))
            .step(
                EffectStep::new(EffectCode::builtin("film_grain_noise", "noise"))
                    .label("noise")
                    .write("target", write("grain"))
                    .uniform("size", EffectUniform::Size)
                    .uniform("time", EffectUniform::Time),
            )
            .step(
                EffectStep::new(EffectCode::builtin("film_grain", "combine"))
                    .label("combine")
                    .read("source", image())
                    .read("grain", read("grain"))
                    .write("target", EffectOutput::Image)
                    .uniform("size", EffectUniform::Size),
            )
    }

    pub(crate) fn chromatic_aberration() -> Self {
        single(BuiltinEffect::ChromaticAberration, "disperse", |step| step)
    }

    pub(crate) fn lens_dirt() -> Self {
        let quarter = || target(EffectFormat::Rgba16Float, 4);
        effect(BuiltinEffect::LensDirt)
            .target("bright", quarter())
            .target("spread", quarter())
            .target("glare", quarter())
            .target("pattern", target(EffectFormat::Rgba16Float, 2))
            .step(pass(
                "lens_dirt_bright",
                "extract",
                image(),
                write("bright"),
            ))
            .step(pass(
                "lens_dirt_blur",
                "horizontal",
                read("bright"),
                write("spread"),
            ))
            .step(pass(
                "lens_dirt_blur",
                "vertical",
                read("spread"),
                write("glare"),
            ))
            .step(
                EffectStep::new(EffectCode::builtin("lens_dirt_pattern", "pattern"))
                    .label("pattern")
                    .write("target", write("pattern"))
                    .uniform("size", EffectUniform::Size),
            )
            .step(
                pass("lens_dirt", "combine", image(), EffectOutput::Image)
                    .read("glare", read("glare"))
                    .read("pattern", read("pattern")),
            )
    }

    pub(crate) fn letterbox() -> Self {
        single(BuiltinEffect::Letterbox, "matte", |step| step)
    }

    pub(crate) fn gate_weave() -> Self {
        effect(BuiltinEffect::GateWeave)
            .target("motion", target(EffectFormat::Rgba16Float, SINGLE_TEXEL))
            .step(
                EffectStep::new(EffectCode::builtin("gate_weave_wander", "wander"))
                    .label("wander")
                    .write("target", write("motion"))
                    .uniform("size", EffectUniform::Size)
                    .uniform("time", EffectUniform::Time),
            )
            .step(
                pass("gate_weave", "weave", image(), EffectOutput::Image)
                    .read("motion", read("motion")),
            )
    }
}
