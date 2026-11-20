use crate::effect::builtin::{
    BuiltinEffect, effect, image, pass, read, socket, step, target, write,
};
use crate::effect::desc::{Effect, EffectFormat, EffectOutput, EffectStep};

pub(crate) const SIZE: &str = "size";
const APERTURE: &str = "aperture";
const LEVELS: u32 = 8;

fn level(index: u32) -> String {
    format!("level{index}")
}

fn levels(step: EffectStep) -> EffectStep {
    (1..=LEVELS).fold(step, |step, index| {
        step.read(level(index), read(&level(index)))
    })
}

impl Effect {
    pub(crate) fn gaussian_blur() -> Self {
        let full = || target(EffectFormat::Rgba16Float, 1);
        let mut effect = (1..=LEVELS).fold(
            effect(BuiltinEffect::GaussianBlur).optional(SIZE),
            |effect, index| {
                effect.target(level(index), target(EffectFormat::Rgba16Float, 1 << index))
            },
        );
        effect = effect.target("spread", full()).target("blurred", full());
        for index in 1..=LEVELS {
            let source = match index {
                1 => image(),
                index => read(&level(index - 1)),
            };
            effect = effect.step(
                pass(
                    "gaussian_blur_reduce",
                    "reduce",
                    source,
                    write(&level(index)),
                )
                .read("image", image())
                .read("size_map", socket(SIZE)),
            );
        }
        effect
            .step(levels(
                step("gaussian_blur_horizontal", "horizontal", write("spread"))
                    .read("image", image())
                    .read("size_map", socket(SIZE)),
            ))
            .step(
                pass(
                    "gaussian_blur_vertical",
                    "vertical",
                    read("spread"),
                    write("blurred"),
                )
                .read("image", image())
                .read("size_map", socket(SIZE)),
            )
            .step(levels(
                step("gaussian_blur", "combine", EffectOutput::Image)
                    .read("image", image())
                    .read("size_map", socket(SIZE))
                    .read("blurred", read("blurred")),
            ))
    }

    pub(crate) fn directional_blur() -> Self {
        effect(BuiltinEffect::DirectionalBlur)
            .target("fine", target(EffectFormat::Rgba16Float, 1))
            .step(pass("directional_blur", "fine", image(), write("fine")))
            .step(pass(
                "directional_blur",
                "coarse",
                read("fine"),
                EffectOutput::Image,
            ))
    }

    pub(crate) fn bokeh_blur() -> Self {
        effect(BuiltinEffect::BokehBlur)
            .optional(SIZE)
            .optional(APERTURE)
            .target("half", target(EffectFormat::Rgba16Float, 2))
            .target("quarter", target(EffectFormat::Rgba16Float, 4))
            .target("eighth", target(EffectFormat::Rgba16Float, 8))
            .target("blurred", target(EffectFormat::Rgba16Float, 2))
            .step(pass(
                "bokeh_blur_prefilter",
                "prefilter",
                image(),
                write("half"),
            ))
            .step(pass(
                "bokeh_blur_prefilter",
                "reduce",
                read("half"),
                write("quarter"),
            ))
            .step(pass(
                "bokeh_blur_prefilter",
                "reduce",
                read("quarter"),
                write("eighth"),
            ))
            .step(
                step("bokeh_blur_gather", "gather", write("blurred"))
                    .read("image", image())
                    .read("size_map", socket(SIZE))
                    .read("aperture", socket(APERTURE))
                    .read("half_level", read("half"))
                    .read("quarter_level", read("quarter"))
                    .read("eighth_level", read("eighth")),
            )
            .step(
                pass("bokeh_blur", "combine", image(), EffectOutput::Image)
                    .read("size_map", socket(SIZE))
                    .read("blurred", read("blurred")),
            )
    }
}
