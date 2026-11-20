use std::f32::consts::PI;

use cut_project::TransitionKind as Kind;
use rikka_image_edit::{
    Blend, Channel, ConvertProfile, Fill, FractalNoise, GaussianBlur, Level, Levels, Mask, Picture,
    Place, Profile, Recipe, Result,
};

use super::Mix;
use crate::plan::IDENTITY;

const MIN_BLUR: f32 = 0.25;
const NOISE_DETAIL: u32 = 4;
const NOISE_CONTRAST: f32 = 200.0;

pub(super) fn render(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    match mix.kind {
        Kind::FilmDissolve => film(mix, out, into),
        Kind::AdditiveDissolve => additive(mix, out, into),
        Kind::Dip => dip(mix, out, into),
        Kind::BlurDissolve => blurred(mix, out, into),
        Kind::NoiseDissolve => noisy(mix, out, into),
        _ => mix.mixed(out, into, mix.progress, None),
    }
}

fn film(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let Some(deep) = &mix.deep else {
        return mix.mixed(out, into, mix.progress, None);
    };
    let linear = |picture: &Picture| {
        deep.develop(
            &Recipe::new()
                .then(Place::new(picture.clone(), IDENTITY))
                .then(ConvertProfile::new(Profile::linear_srgb())),
        )
    };
    mix.mixed(&linear(out)?, &linear(into)?, mix.progress, None)
}

fn additive(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let progress = mix.progress;
    let leaving = (2.0 * (1.0 - progress)).min(1.0);
    let arriving = (2.0 * progress).min(1.0);
    mix.clear.develop(
        &Recipe::new()
            .then(Place::new(out.clone(), IDENTITY).opacity(leaving))
            .then(
                Place::new(into.clone(), IDENTITY)
                    .opacity(arriving)
                    .blend(Blend::LinearDodge),
            ),
    )
}

fn dip(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let colour = mix
        .clear
        .develop(&Recipe::new().then(Fill::new(mix.tuning.tint)))?;
    match mix.progress < 0.5 {
        true => mix.mixed(out, &colour, mix.progress * 2.0, None),
        false => mix.mixed(&colour, into, mix.progress * 2.0 - 1.0, None),
    }
}

fn blurred(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let sigma = mix.tuning.blur * mix.factor * (PI * mix.progress).sin();
    let blur = |picture: &Picture| match sigma > MIN_BLUR {
        true => picture.develop(&Recipe::new().then(GaussianBlur::new(sigma))),
        false => Ok(picture.clone()),
    };
    mix.mixed(&blur(out)?, &blur(into)?, mix.progress, None)
}

fn noisy(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let progress = mix.progress;
    if progress <= 0.0 {
        return Ok(out.clone());
    }
    if progress >= 1.0 {
        return Ok(into.clone());
    }
    let soft = (mix.tuning.softness / 100.0).max(0.01);
    let spread = 1.0 + 2.0 * soft;
    let edge = progress * (1.0 + soft);
    let matte = mix.clear.develop(
        &Recipe::new()
            .then(
                FractalNoise::new(mix.tuning.size * mix.factor)
                    .detail(NOISE_DETAIL)
                    .seed(mix.seed)
                    .contrast(NOISE_CONTRAST),
            )
            .then(Levels::new().channel(
                Channel::Composite,
                Level::new().output(soft / spread, (1.0 + soft) / spread),
            ))
            .then(Levels::new().channel(
                Channel::Composite,
                Level::new().input((edge + soft) / spread, edge / spread),
            )),
    )?;
    mix.mixed(out, into, 1.0, Some(Mask::luma(matte)))
}
