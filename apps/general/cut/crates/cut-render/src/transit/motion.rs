use std::f32::consts::PI;

use cut_project::{Direction, TransitionKind as Kind};
use rikka_image_edit::{MotionBlur, Picture, Place, RadialBlur, Recipe, Result};

use super::{Change, Mix, degrees, smooth};
use crate::plan::IDENTITY;

const MIN_SMEAR: f32 = 0.5;
const CROSS_ZOOM: f32 = 0.6;
const TURN_DEPTH: f32 = 2.0;
const EDGE_ON: f32 = 89.5;

pub(super) fn render(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    match mix.kind {
        Kind::Push => push(mix, out, into),
        Kind::SlideIn => slide_in(mix, out, into),
        Kind::SlideOut => slide_out(mix, out, into),
        Kind::CrossZoom => cross_zoom(mix, out, into),
        _ => turn(mix, out, into),
    }
}

pub(super) fn smeared(picture: &Picture, direction: Direction, distance: f32) -> Result<Picture> {
    match distance > MIN_SMEAR {
        true => picture.develop(&Recipe::new().then(MotionBlur::new(degrees(direction), distance))),
        false => Ok(picture.clone()),
    }
}

fn push(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let span = mix.travel();
    let progress = mix.progress;
    let pushed = mix.clear.develop(
        &Recipe::new()
            .then(Place::new(out.clone(), mix.shifted(span * progress)))
            .then(Place::new(
                into.clone(),
                mix.shifted(span * (progress - 1.0)),
            )),
    )?;
    smeared(&pushed, mix.direction, mix.sweep())
}

fn slide_in(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let shift = mix.shifted(mix.travel() * (mix.progress - 1.0));
    let arriving = smeared(&mix.moved(into, shift, false)?, mix.direction, mix.sweep())?;
    out.develop(&Recipe::new().then(Place::new(arriving, IDENTITY)))
}

fn slide_out(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let shift = mix.shifted(mix.travel() * mix.progress);
    let leaving = smeared(&mix.moved(out, shift, false)?, mix.direction, mix.sweep())?;
    into.develop(&Recipe::new().then(Place::new(leaving, IDENTITY)))
}

fn cross_zoom(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let progress = mix.progress;
    let leaving = mix.moved(
        out,
        mix.about(Change::zoom(1.0 + CROSS_ZOOM * progress)),
        mix.fills[0],
    )?;
    let arriving = mix.moved(
        into,
        mix.about(Change::zoom(1.0 + CROSS_ZOOM * (1.0 - progress))),
        mix.fills[1],
    )?;
    let transition = mix.mixed(&leaving, &arriving, smooth(0.4, 0.6, progress), None)?;
    let strength = mix.amount() * (PI * progress).sin();
    match strength > MIN_SMEAR {
        true => transition.develop(&Recipe::new().then(RadialBlur::zoom(strength))),
        false => Ok(transition),
    }
}

fn turn(mix: &Mix, out: &Picture, into: &Picture) -> Result<Picture> {
    let angle = mix.progress * 180.0;
    let (side, angle) = match angle < 90.0 {
        true => (out, angle),
        false => (into, angle - 180.0),
    };
    if angle.abs() > EDGE_ON {
        return Ok(mix.clear.clone());
    }
    let sign = match mix.direction {
        Direction::Right | Direction::Down => -1.0,
        _ => 1.0,
    };
    let (sin, cos) = (sign * angle).to_radians().sin_cos();
    let depth = TURN_DEPTH * mix.reach();
    let lens = match mix.direction {
        Direction::Up | Direction::Down => [
            [depth, 0.0, 0.0],
            [0.0, depth * cos, 0.0],
            [0.0, sin, depth],
        ],
        _ => [
            [depth * cos, 0.0, 0.0],
            [0.0, depth, 0.0],
            [sin, 0.0, depth],
        ],
    };
    let (cx, cy) = mix.centre();
    let to = [[1.0, 0.0, cx], [0.0, 1.0, cy], [0.0, 0.0, 1.0]];
    let from = [[1.0, 0.0, -cx], [0.0, 1.0, -cy], [0.0, 0.0, 1.0]];
    let matrix = times(&times(&to, &lens), &from);
    mix.moved(side, matrix, false)
}

fn times(left: &[[f32; 3]; 3], right: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut product = [[0.0; 3]; 3];
    for (row, row_values) in product.iter_mut().enumerate() {
        for (column, value) in row_values.iter_mut().enumerate() {
            *value = (0..3).map(|at| left[row][at] * right[at][column]).sum();
        }
    }
    product
}
