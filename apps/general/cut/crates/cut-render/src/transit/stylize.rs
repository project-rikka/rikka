use cut_project::TransitionKind as Kind;
use rikka_image_edit::{
    Blend, Channel, ChannelShift, Displace, Exposure, Fill, FractalNoise, GradientMap,
    GradientStop, Level, Levels, Mask, Mosaic, Picture, Place, Recipe, Result, Step,
};
use rikka_types::Color;

use super::{Change, Mix, hashed, smooth};
use crate::plan::IDENTITY;

const MIN_SHIFT: f32 = 0.5;
const GLITCH_BANDS: u64 = 8;
const GLITCH_SHIFT: f32 = 0.15;
const GLITCH_SPLIT: f32 = 24.0;
const GLITCH_FLICKER: f32 = 0.5;
const CHROMA_ZOOM: f32 = 0.08;
const FLASH_STOPS: f32 = 3.0;
const FLASH_COVER: f32 = 0.9;
const LEAK_STOPS: f32 = 0.7;
const LEAK_GLOW: f32 = 1.4;
const BURN_STOPS: f32 = 1.0;
const BURN_REACH: f32 = 0.95;

pub(super) fn render(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    match mix.kind {
        Kind::Glitch => glitch(mix, out, into),
        Kind::ChromaSplit => chroma(mix, out, into),
        Kind::Flash => flash(mix, out, into),
        Kind::LightLeak => leak(mix, out, into),
        Kind::FilmBurn => burn(mix, out, into),
        Kind::Pixelate => pixelate(mix, out, into),
        _ => liquid(mix, out, into),
    }
}

fn glitch(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let peak = mix.swell();
    let strength = mix.strength();
    let flicker = hashed(mix.seed, (mix.step as u64).wrapping_mul(7) + 11) < peak * GLITCH_FLICKER;
    let early = mix.progress < 0.5;
    let (side, fills) = match early != flicker {
        true => (out, mix.fills[0]),
        false => (into, mix.fills[1]),
    };
    let split = GLITCH_SPLIT * mix.factor * peak * strength;
    let side = split_and_grown(mix, side, split, 0.0, fills)?;
    let (width, height) = (mix.width as f32, mix.height as f32);
    let mut recipe = Recipe::new();
    for band in 0..GLITCH_BANDS {
        let draw = |part: u64| {
            let salt = (mix.step as u64).wrapping_mul(GLITCH_BANDS) + band;
            hashed(mix.seed, salt.wrapping_mul(5) + part)
        };
        if draw(0) >= peak * strength {
            continue;
        }
        let top = draw(1) * height;
        let tall = (0.02 + 0.1 * draw(2)) * height;
        let shift = (draw(3) * 2.0 - 1.0) * GLITCH_SHIFT * width * peak * strength;
        let place = Place::new(
            side.clone(),
            mix.about(Change {
                shift: (shift, 0.0),
                ..Change::NONE
            }),
        )
        .replacing();
        let place = match fills {
            true => place.mirrored(),
            false => place,
        };
        recipe.push(Step::new(place).masked(Mask::rectangle(0.0, top, width, tall)));
    }
    side.develop(&recipe)
}

fn split_and_grown(
    mix: &Mix,
    side: &Picture,
    split: f32,
    grow: f32,
    fills: bool,
) -> Result<Picture> {
    let least = mix.width.min(mix.height).max(1) as f32;
    let split_apart = match split > MIN_SHIFT {
        true => side.develop(&Recipe::new().then(ChannelShift::new(0.0, split)))?,
        false => side.clone(),
    };
    let zoom = 1.0 + 2.0 * split / least + grow;
    mix.moved(&split_apart, mix.about(Change::zoom(zoom)), fills)
}

fn chroma(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let peak = mix.swell();
    let split = mix.amount() * mix.factor * peak;
    split_and_grown(mix, side, split, CHROMA_ZOOM * peak, fills)
}

fn flash(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, _) = mix.side(out, into);
    let peak = mix.peak().powf(1.5);
    let strength = mix.strength();
    side.develop(
        &Recipe::new()
            .then(Exposure::new(peak * FLASH_STOPS * strength))
            .then(
                Step::new(Fill::new(mix.tuning.tint))
                    .opacity((peak * strength).min(1.0) * FLASH_COVER),
            ),
    )
}

fn glowing(light: &Picture, opacity: f32, blend: Blend) -> Step {
    Step::new(Place::new(light.clone(), IDENTITY).blend(blend))
        .opacity(opacity.clamp(0.0, 1.0))
        .masked(Mask::luma(light.clone()))
}

fn leak(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let peak = mix.swell();
    let strength = mix.strength();
    let transition = mix.mixed(out, into, smooth(0.42, 0.58, mix.progress), None)?;
    let light = mix.clear.develop(
        &Recipe::new()
            .then(
                FractalNoise::new(0.6 * mix.reach())
                    .detail(2)
                    .seed(mix.seed)
                    .evolution(mix.linear * 1.5)
                    .offset(mix.linear * mix.width as f32 * 0.8, 0.0)
                    .contrast(220.0),
            )
            .then(GradientMap::new([
                GradientStop::new(0.0, Color::BLACK),
                GradientStop::new(0.55, mix.tuning.tint),
                GradientStop::new(1.0, Color::WHITE),
            ])),
    )?;
    transition.develop(
        &Recipe::new()
            .then(glowing(&light, peak * strength * LEAK_GLOW, Blend::Screen))
            .then(Exposure::new(peak * LEAK_STOPS * strength)),
    )
}

fn burn(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let peak = mix.swell().powf(1.5);
    let strength = mix.strength();
    let transition = mix.mixed(out, into, smooth(0.45, 0.55, mix.progress), None)?;
    let threshold = 1.0 - peak * BURN_REACH * strength.min(1.5);
    let heat = mix.clear.develop(
        &Recipe::new()
            .then(
                FractalNoise::new(0.3 * mix.reach())
                    .detail(5)
                    .seed(mix.seed)
                    .evolution(mix.linear * 0.8)
                    .contrast(180.0),
            )
            .then(Levels::new().channel(
                Channel::Composite,
                Level::new().input(threshold, threshold + 0.3),
            )),
    )?;
    let fire = heat.develop(&Recipe::new().then(GradientMap::new([
        GradientStop::new(0.0, Color::BLACK),
        GradientStop::new(0.45, mix.tuning.tint),
        GradientStop::new(0.8, Color::rgb(1.0, 0.9, 0.7)),
        GradientStop::new(1.0, Color::WHITE),
    ])))?;
    transition.develop(
        &Recipe::new()
            .then(
                Step::new(Place::new(fire, IDENTITY).blend(Blend::LinearDodge))
                    .masked(Mask::luma(heat)),
            )
            .then(Exposure::new(peak * peak * BURN_STOPS * strength)),
    )
}

fn pixelate(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let transition = mix.mixed(out, into, smooth(0.45, 0.55, mix.progress), None)?;
    let cell = 1.0 + (mix.amount() * mix.factor - 1.0).max(0.0) * mix.swell();
    match cell >= 1.5 {
        true => transition.develop(&Recipe::new().then(Mosaic::new(cell))),
        false => Ok(transition),
    }
}

fn liquid(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let peak = mix.swell();
    let transition = mix.mixed(out, into, smooth(0.4, 0.6, mix.progress), None)?;
    let reach = mix.amount() * mix.factor * peak;
    if reach <= MIN_SHIFT {
        return Ok(transition);
    }
    let map = mix.clear.develop(
        &Recipe::new().then(
            FractalNoise::new(mix.tuning.size * mix.factor)
                .detail(3)
                .seed(mix.seed)
                .evolution(mix.linear * 2.0)
                .colored(),
        ),
    )?;
    transition.develop(&Recipe::new().then(Displace::new(map, reach, reach)))
}
