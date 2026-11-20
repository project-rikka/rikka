use crate::effect::builtin::{BuiltinEffect, effect, image, single, target, write};
use crate::effect::desc::{
    Effect, EffectCode, EffectFormat, EffectInput, EffectOutput, EffectStep, EffectUniform,
};

impl Effect {
    pub(crate) fn trails() -> Self {
        single(BuiltinEffect::Trails, "echo", |step| {
            step.read("history", EffectInput::History)
        })
    }

    pub(crate) fn frame_blend() -> Self {
        effect(BuiltinEffect::FrameBlend)
            .target("last", target(EffectFormat::Rgba16Float, 1).kept())
            .step(
                EffectStep::new(EffectCode::builtin("frame_blend", "blend"))
                    .label("blend")
                    .read("source", image())
                    .read("last", EffectInput::Kept("last".to_owned()))
                    .write("target", EffectOutput::Image)
                    .write("keep", write("last"))
                    .uniform("size", EffectUniform::Size),
            )
    }

    pub(crate) fn datamosh() -> Self {
        single(BuiltinEffect::Datamosh, "mosh", |step| {
            step.read("history", EffectInput::History)
                .read("velocity", EffectInput::Velocity)
                .uniform("time", EffectUniform::Time)
        })
    }
}
