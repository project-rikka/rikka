use std::f32::consts::TAU;

use glam::{Quat, Vec3};
use rikka_physics_tailor::Style;

use crate::inputs::Joint;
use crate::layout::PieceRecord;
use crate::layout::Topology;

const TELEPORT_SPEED: f32 = 20.0;
const TELEPORT_TURN: f32 = 60.0;
const TRAVEL_PER_SUBSTEP: f32 = 0.004;

pub(crate) struct InertiaContext<'a> {
    pub(crate) topology: &'a Topology,
    pub(crate) styles: &'a [Style],
    pub(crate) gravity: Vec3,
    pub(crate) dt: f32,
    pub(crate) carried_whole: bool,
}

impl InertiaContext<'_> {
    pub(crate) fn pieces(
        &self,
        previous: &[Joint],
        current: &[Joint],
        smoothed: &mut [Vec3],
    ) -> Vec<PieceRecord> {
        self.topology
            .frames
            .iter()
            .zip(self.styles)
            .zip(&self.topology.layers)
            .zip(&self.topology.layer_triangles)
            .zip(&self.topology.sheets)
            .zip(smoothed.iter_mut())
            .map(
                |(((((frame, style), layer), layer_triangles), sheet), smoothed)| {
                    let from = previous[frame.slot as usize];
                    let to = current[frame.slot as usize];
                    let centre = from.point(frame.centre);
                    let moved = to.point(frame.centre) - centre;
                    let (axis, angle) = turn(from.rotation, to.rotation);
                    let felt = if self.carried_whole {
                        *smoothed = Vec3::ZERO;
                        InertiaResponse::carried(moved)
                    } else {
                        InertiaResponse::compute(style, moved, angle, smoothed, self.dt)
                    };
                    PieceRecord {
                        centre: centre.extend(felt.turned).to_array(),
                        scaled: felt.scaled.extend(felt.moved).to_array(),
                        turn: axis.extend(angle).to_array(),
                        fixed_shift: felt
                            .fixed
                            .extend(style.gravity * self.falloff(style, to.rotation))
                            .to_array(),
                        spin: (axis * (angle * (1.0 - felt.turned.max(0.0)) / self.dt))
                            .extend(style.inertia.centrifugal)
                            .to_array(),
                        response: [
                            style.attenuation.clamp(0.0, 1.0),
                            finite_or_most(style.max_speed),
                            style.drag.max(0.0),
                            style.lift.max(0.0),
                        ],
                        flutter: [
                            style.flutter.amplitude * (TAU * style.flutter.frequency).powi(2),
                            style.flutter.frequency,
                            style.flutter.wavelength,
                            0.0,
                        ],
                        contact: [
                            f32::from(u8::from(*sheet)),
                            style.friction.clamp(0.0, 1.0),
                            f32::from(*layer),
                            f32::from(u8::from(style.self_collision)),
                        ],
                        settling: [style.settling.clamp(0.0, 1.0), 0.0, 0.0, 0.0],
                        layer_triangles: *layer_triangles,
                        grab: [0.0; 4],
                    }
                },
            )
            .collect()
    }

    fn falloff(&self, style: &Style, rotation: Quat) -> f32 {
        let down = self.gravity.normalize_or_zero();
        let upright = ((rotation * down).dot(down) * 0.5 + 0.5).clamp(0.0, 1.0);
        let falloff = style.gravity_falloff.clamp(0.0, 1.0);
        (1.0 - falloff) + falloff * (1.0 - upright)
    }

    pub(crate) fn substeps(
        &self,
        previous: &[Joint],
        current: &[Joint],
        range: (u32, u32),
        epochs: u32,
    ) -> u32 {
        let travel = previous
            .iter()
            .zip(current)
            .zip(&self.topology.reach)
            .map(|((from, to), reach)| {
                let (_, angle) = turn(from.rotation, to.rotation);
                from.translation.distance(to.translation) + angle * reach
            })
            .fold(0.0f32, f32::max);
        let needed = (travel / TRAVEL_PER_SUBSTEP).ceil();
        let substeps = if needed.is_finite() {
            (needed as u32).clamp(range.0, range.1)
        } else {
            range.1
        };
        substeps.next_multiple_of(epochs)
    }
}

pub(crate) fn jumped(topology: &Topology, previous: &[Joint], current: &[Joint], dt: f32) -> bool {
    topology.frames.iter().any(|frame| {
        let from = previous[frame.slot as usize];
        let to = current[frame.slot as usize];
        let moved = to.point(frame.centre) - from.point(frame.centre);
        let (_, angle) = turn(from.rotation, to.rotation);
        moved.length() > TELEPORT_SPEED * dt || angle > TELEPORT_TURN * dt
    })
}

struct InertiaResponse {
    scaled: Vec3,
    fixed: Vec3,
    moved: f32,
    turned: f32,
}

impl InertiaResponse {
    fn carried(moved: Vec3) -> InertiaResponse {
        InertiaResponse {
            scaled: Vec3::ZERO,
            fixed: moved,
            moved: 1.0,
            turned: 1.0,
        }
    }

    fn compute(
        style: &Style,
        moved: Vec3,
        angle: f32,
        smoothed: &mut Vec3,
        dt: f32,
    ) -> InertiaResponse {
        let inertia = &style.inertia;
        let speed_limit = limit(inertia.speed_limit);
        let (scaled, fixed) = if inertia.smoothing > 0.0 {
            let raw = (moved / dt).clamp_length_max(speed_limit);
            let ratio =
                ((1.0 - inertia.smoothing.clamp(0.0, 1.0)).powi(3) * 0.99 + 0.01).clamp(0.0, 1.0);
            *smoothed = smoothed.lerp(raw, ratio);
            (*smoothed * dt, moved - *smoothed * dt)
        } else {
            *smoothed = Vec3::ZERO;
            (moved, Vec3::ZERO)
        };
        let mut carried = inertia.carried.clamp(-1.0, 1.0);
        let felt_speed = scaled.length() * (1.0 - carried) / dt;
        if felt_speed > speed_limit {
            carried += (1.0 - carried) * ((felt_speed - speed_limit) / felt_speed);
        }
        let mut turned = inertia.carried_turn.clamp(-1.0, 1.0);
        let turn_limit = limit(inertia.turn_limit);
        let felt_turn = angle * (1.0 - turned) / dt;
        if felt_turn > turn_limit {
            turned += (1.0 - turned) * ((felt_turn - turn_limit) / felt_turn);
        }
        InertiaResponse {
            scaled,
            fixed,
            moved: carried,
            turned,
        }
    }
}

fn limit(value: f32) -> f32 {
    if value >= 0.0 { value } else { f32::INFINITY }
}

fn finite_or_most(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        f32::MAX
    }
}

pub(crate) fn turn(from: Quat, to: Quat) -> (Vec3, f32) {
    let mut delta = (to * from.inverse()).normalize();
    if delta.w < 0.0 {
        delta = -delta;
    }
    let sine = delta.xyz().length();
    if sine <= 1.0e-9 {
        return (Vec3::Y, 0.0);
    }
    (delta.xyz() / sine, 2.0 * sine.atan2(delta.w))
}
