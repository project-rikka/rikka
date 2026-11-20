use animate_fluid::{Emitter, Shape};
use rikka_model::glam::{Mat4, Vec3};
use rikka_physics::{EmitterShape, EmitterState, Launch};

use super::grid::frame_at_step;

const LITRE: f32 = 1e-3;

pub(super) fn emitter_state(
    emitter: &Emitter,
    step: u32,
    gravity: Vec3,
    locate: impl Fn(&str, &str) -> Option<Mat4>,
) -> EmitterState {
    let state = emitter.sample(frame_at_step(step));
    let launch = match state.shape.launches() {
        true => state.launch.world(&locate),
        false => None,
    };
    let target = match state.shape.aims() {
        true => state.target.world(&locate),
        false => None,
    };
    let levels = state.levels;
    let radius = levels.width * 0.5;
    let shape = match state.shape {
        Shape::Stream => EmitterShape::Stream { radius },
        Shape::Shower => EmitterShape::Shower {
            radius,
            spread: (levels.spread * 0.5).to_radians(),
        },
        Shape::Drip => EmitterShape::Drip { radius },
        Shape::Blob { .. } => EmitterShape::Blob,
        Shape::Surface { .. } => EmitterShape::Surface,
    };
    EmitterState {
        id: emitter.id.0,
        shape,
        enabled: state.on,
        rate: levels.rate * LITRE,
        volume: levels.volume * LITRE,
        launch: launch.map(|position| Launch {
            position,
            velocity: state
                .flight_between(position, target, gravity)
                .map(|flight| flight.throw.velocity),
        }),
        bursts: bursts(emitter, step),
    }
}

fn bursts(emitter: &Emitter, step: u32) -> u32 {
    let after = match step.checked_sub(1) {
        Some(before) => frame_at_step(before),
        None => f32::NEG_INFINITY,
    };
    u32::from(emitter.lands_between(after, frame_at_step(step)))
}
