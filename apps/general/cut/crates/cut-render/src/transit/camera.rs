use cut_project::{Direction, TransitionKind as Kind};
use rikka_image_edit::{LensDistortion, MotionBlur, Picture, RadialBlur, Recipe, Result};

use super::motion::smeared;
use super::{Change, Mix, hashed};

const MIN_BLUR: f32 = 0.5;
const MAX_ZOOM_BLUR: f32 = 100.0;
const MAX_SPIN_BLUR: f32 = 360.0;
const SPIN_ZOOM: f32 = 0.1;
const SHAKE_TURN: f32 = 2.0;
const SHAKE_SALT: u64 = 3;
const LENS_ZOOM: f32 = 0.35;
const STRETCH_THINNING: f32 = 0.25;

pub(super) fn render(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    match mix.kind {
        Kind::ZoomIn => zoom(mix, out, into, 1.0),
        Kind::ZoomOut => zoom(mix, out, into, -1.0),
        Kind::Spin => spin(mix, out, into),
        Kind::WhipPan => whip(mix, out, into),
        Kind::Stretch => stretch(mix, out, into),
        Kind::Shake => shake(mix, out, into),
        _ => lens(mix, out, into),
    }
}

fn phase(progress: f32) -> f32 {
    match progress < 0.5 {
        true => progress,
        false => progress - 1.0,
    }
}

fn zoom_blurred(picture: Picture, strength: f32) -> Result<Picture> {
    match strength > MIN_BLUR {
        true => picture.develop(&Recipe::new().then(RadialBlur::zoom(strength.min(MAX_ZOOM_BLUR)))),
        false => Ok(picture),
    }
}

fn zoom(mix: &Mix, out: &Picture, into: &Picture, sign: f32) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let reach = mix.strength().max(1.01).ln();
    let scale = (sign * 2.0 * reach * phase(mix.progress)).exp();
    let zoomed = mix.moved(side, mix.about(Change::zoom(scale)), fills)?;
    let rate = 2.0 * reach * mix.per_frame(|progress| progress);
    zoom_blurred(zoomed, 200.0 * rate * mix.shutter())
}

fn spin(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let sign = match mix.direction {
        Direction::Left => -1.0,
        _ => 1.0,
    };
    let total = mix.amount();
    let grown = 1.0 + SPIN_ZOOM * mix.peak();
    let turned = mix.moved(
        side,
        mix.about(Change {
            turn: sign * total * phase(mix.progress),
            ..Change::zoom(grown)
        }),
        fills,
    )?;
    let blur = (total * mix.per_frame(|progress| progress) * mix.shutter()).min(MAX_SPIN_BLUR);
    match blur > MIN_BLUR {
        true => turned.develop(&Recipe::new().then(RadialBlur::spin(blur))),
        false => Ok(turned),
    }
}

fn whip(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let shift = mix.shifted(mix.travel() * phase(mix.progress));
    smeared(&mix.moved(side, shift, fills)?, mix.direction, mix.sweep())
}

fn stretch(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let reach = mix.strength().max(1.01).ln();
    let along = (2.0 * reach * phase(mix.progress).abs()).exp();
    let thinner = along.powf(-STRETCH_THINNING);
    let (scale, direction) = match mix.direction {
        Direction::Up | Direction::Down => ((thinner, along), Direction::Up),
        _ => ((along, thinner), Direction::Right),
    };
    let span = mix.travel();
    let stretched = mix.moved(
        side,
        mix.about(Change {
            scale,
            ..Change::NONE
        }),
        fills,
    )?;
    let smear =
        along * 2.0 * reach * mix.per_frame(|progress| progress) * span / 2.0 * mix.shutter();
    smeared(&stretched, direction, smear)
}

fn wobble(mix: &Mix, step: i64) -> (f32, f32, f32) {
    let salt = |part: u64| (step as u64).wrapping_mul(SHAKE_SALT).wrapping_add(part);
    let signed = |part: u64| hashed(mix.seed, salt(part)) * 2.0 - 1.0;
    (signed(0), signed(1), signed(2))
}

fn shake(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let peak = mix.swell();
    let swing = mix.amount() * mix.factor * peak;
    let (x, y, turn) = wobble(mix, mix.step);
    let (was_x, was_y, _) = wobble(mix, mix.step - 1);
    let least = mix.width.min(mix.height).max(1) as f32;
    let shaken = mix.moved(
        side,
        mix.about(Change {
            shift: (x * swing, y * swing),
            turn: turn * SHAKE_TURN * peak,
            ..Change::zoom(1.0 + 2.0 * swing / least)
        }),
        fills,
    )?;
    let (dx, dy) = ((x - was_x) * swing, (y - was_y) * swing);
    let smear = dx.hypot(dy) * mix.shutter();
    match smear > MIN_BLUR {
        true => shaken
            .develop(&Recipe::new().then(MotionBlur::new((-dy).atan2(dx).to_degrees(), smear))),
        false => Ok(shaken),
    }
}

fn lens(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let (side, fills) = mix.side(out, into);
    let peak = mix.peak();
    let strength = mix.strength();
    let grown = mix.moved(
        side,
        mix.about(Change::zoom(1.0 + LENS_ZOOM * strength * peak)),
        fills,
    )?;
    let bent = grown.develop(&Recipe::new().then(LensDistortion::new(peak * mix.amount())))?;
    let rate = 2.0 * LENS_ZOOM * strength * mix.per_frame(|progress| progress);
    zoom_blurred(bent, 200.0 * rate * mix.shutter())
}
