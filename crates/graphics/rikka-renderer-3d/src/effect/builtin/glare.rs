use crate::effect::builtin::{BuiltinEffect, effect, image, pass, read, step, target, write};
use crate::effect::desc::{
    Effect, EffectCode, EffectFormat, EffectOutput, EffectStep, EffectUniform,
};

const STREAK_LEVELS: [(&str, u32); 4] =
    [("base", 4), ("middle", 8), ("coarse", 16), ("coarsest", 32)];

fn reduce(from: &str, to: &str) -> EffectStep {
    EffectStep::new(EffectCode::builtin("pyramid_reduce", "reduce"))
        .label("reduce")
        .read("source", read(from))
        .write("target", write(to))
        .uniform("size", EffectUniform::Size)
}

fn rays(effect: Effect, extract: &str) -> Effect {
    let effect = STREAK_LEVELS
        .iter()
        .fold(effect, |effect, &(name, divisor)| {
            effect.target(name, target(EffectFormat::Rgba16Float, divisor))
        });
    effect
        .step(pass(extract, "extract", image(), write("base")))
        .step(reduce("base", "middle"))
        .step(reduce("middle", "coarse"))
        .step(reduce("coarse", "coarsest"))
}

fn levels(step: EffectStep) -> EffectStep {
    STREAK_LEVELS
        .iter()
        .fold(step, |step, &(name, _)| step.read(name, read(name)))
}

impl Effect {
    pub(crate) fn streaks() -> Self {
        rays(effect(BuiltinEffect::Streaks), "streaks_bright")
            .target("rays", target(EffectFormat::Rgba16Float, 4))
            .step(levels(
                step("streaks_gather", "streak", write("rays")).read("image", image()),
            ))
            .step(
                pass("streaks", "combine", image(), EffectOutput::Image)
                    .read("streaks", read("rays")),
            )
    }

    pub(crate) fn fog_glow() -> Self {
        let level = |index: u32| format!("level{index}");
        let mut effect = (1..=8).fold(effect(BuiltinEffect::FogGlow), |effect, index| {
            effect.target(level(index), target(EffectFormat::Rgba16Float, 1 << index))
        });
        effect = effect
            .target("folded", target(EffectFormat::Rgba16Float, 2))
            .step(pass(
                "fog_glow_bright",
                "extract",
                image(),
                write(&level(1)),
            ));
        for index in 2..=8 {
            effect = effect.step(reduce(&level(index - 1), &level(index)));
        }
        let fold = (1..=8).fold(
            step("fog_glow_fold", "fold", write("folded")).read("image", image()),
            |step, index| step.read(level(index), read(&level(index))),
        );
        effect.step(fold).step(
            pass("fog_glow", "combine", image(), EffectOutput::Image)
                .read("folded", read("folded")),
        )
    }

    pub(crate) fn lens_flare() -> Self {
        let quarter = || target(EffectFormat::Rgba16Float, 4);
        rays(effect(BuiltinEffect::LensFlare), "lens_flare_bright")
            .target("ghosts", quarter())
            .target("spread", quarter())
            .target("soft", quarter())
            .target("spikes", quarter())
            .step(pass(
                "lens_flare_ghosts",
                "ghosts",
                read("base"),
                write("ghosts"),
            ))
            .step(pass(
                "lens_flare_blur",
                "horizontal",
                read("ghosts"),
                write("spread"),
            ))
            .step(pass(
                "lens_flare_blur",
                "vertical",
                read("spread"),
                write("soft"),
            ))
            .step(levels(
                step("lens_flare_spikes", "spikes", write("spikes")).read("image", image()),
            ))
            .step(
                pass("lens_flare", "combine", image(), EffectOutput::Image)
                    .read("ghosts", read("soft"))
                    .read("spikes", read("spikes")),
            )
    }

    pub(crate) fn halation() -> Self {
        let level = |index: u32| format!("level{index}");
        let mut effect = (1..=6).fold(effect(BuiltinEffect::Halation), |effect, index| {
            effect.target(level(index), target(EffectFormat::Rgba16Float, 1 << index))
        });
        effect = effect.step(pass(
            "halation_bright",
            "extract",
            image(),
            write(&level(1)),
        ));
        for index in 2..=6 {
            effect = effect.step(reduce(&level(index - 1), &level(index)));
        }
        let combine = (1..=6).fold(
            pass("halation", "combine", image(), EffectOutput::Image),
            |step, index| step.read(level(index), read(&level(index))),
        );
        effect.step(combine)
    }
}
