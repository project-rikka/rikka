use animate_body::{
    Body, Dof, HumanBone, HumanPose, Limb, Muscle, Worlds, arc, muscle_of, muscles, muscles_of,
    twist_about,
};
use rikka_model_core::glam::{Quat, Vec3};

use crate::dls::Solver;
use crate::effector::Goal;
use crate::turns::{across, is_short, parent_turn, signed_angle};

const FLAT_POLE: f32 = 0.05;
const LIMIT_SLACK: f32 = 1e-3;
const CONVERGED: f32 = 1e-5;
const TWIST_SEARCH: f32 = 0.6;
const TWIST_STEPS: usize = 16;
const FOLLOW_PASSES: usize = 3;
const POLISHED: f32 = 1e-4;
const POLISH_ITERATIONS: usize = 12;

struct Folded {
    span: Vec3,
    upper: f32,
    lower: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tip {
    pub(crate) limb: Limb,
    pub(crate) grip: Vec3,
    pub(crate) goal: Vec3,
    pub(crate) turn: Option<Quat>,
}

#[must_use]
pub fn elbow(root: Vec3, goal: Vec3, upper: f32, lower: f32, pole: Vec3) -> Vec3 {
    let Some(toward) = (goal - root).try_normalize() else {
        return root + pole.normalize_or_zero() * upper;
    };
    let reach = (goal - root)
        .length()
        .clamp((upper - lower).abs(), upper + lower);
    let side = across(pole, toward)
        .try_normalize()
        .unwrap_or_else(|| toward.any_orthonormal_vector());
    let cosine = match upper * reach > 0.0 {
        true => ((upper * upper + reach * reach - lower * lower) / (2.0 * upper * reach))
            .clamp(-1.0, 1.0),
        false => 1.0,
    };
    let sine = (1.0 - cosine * cosine).max(0.0).sqrt();
    root + (toward * cosine + side * sine) * upper
}

pub fn reach(
    body: &Body,
    pose: &mut HumanPose,
    limb: Limb,
    goal_position: Vec3,
    goal_rotation: Option<Quat>,
    weight: f32,
) {
    let weight = weight.clamp(0.0, 1.0);
    if weight <= 0.0 {
        return;
    }
    let before = *pose;
    let mut solved = *pose;
    let worlds = body.fk(&solved);
    solve(
        body,
        &mut solved,
        &worlds,
        limb,
        goal_position,
        goal_rotation,
    );
    for bone in limb.chain() {
        for index in muscles_of(bone) {
            pose.muscles[index] =
                before.muscles[index] + (solved.muscles[index] - before.muscles[index]) * weight;
        }
    }
}

pub(crate) fn follow(
    body: &Body,
    pose: &mut HumanPose,
    tip: Tip,
    around: &HumanPose,
    limits: bool,
) {
    let [upper, lower, end] = tip.limb.chain();
    if !(body.has(upper) && body.has(lower) && body.has(end)) {
        return;
    }
    let range = |angles: (f32, f32), value: f32| (angles.0.min(value), angles.1.max(value));
    let start_bend = muscle_of(lower, Dof::Bend)
        .map_or(0.0, |index| muscles()[index].angle(around.muscles[index]));
    let mut reachable = false;
    for _ in 0..FOLLOW_PASSES {
        let worlds = body.fk(pose);
        let root = worlds.position(upper);
        let Some(direction) = (tip.goal - root).try_normalize() else {
            break;
        };
        let reach = tip.goal.distance(root);
        let Some(folded) = fold(body, pose, [upper, lower, end], tip.grip, reach, |muscle| {
            range(muscle.angles(limits), start_bend)
        }) else {
            break;
        };
        reachable = reach <= folded.upper + folded.lower;
        let upper_rotation = worlds.rotation(upper);
        let aimed = arc(upper_rotation * folded.span, direction) * upper_rotation;
        let parent = parent_turn(body, &worlds, upper);
        body.set_local_rotation(upper, (parent.inverse() * aimed).normalize(), pose);
        body.clamp_around(pose, around, limits);
    }
    if reachable {
        polish(body, pose, tip, around, limits);
    }
    if let Some(turn) = tip.turn {
        orient(body, pose, lower, end, turn.normalize());
        body.clamp_around(pose, around, limits);
    }
}

fn polish(body: &Body, pose: &mut HumanPose, tip: Tip, around: &HumanPose, limits: bool) {
    let [upper, lower, end] = tip.limb.chain();
    let miss = body.fk(pose).point(end, tip.grip).distance(tip.goal);
    if miss <= POLISHED * body.scale() {
        return;
    }
    let variables = muscles_of(upper)
        .chain(muscle_of(lower, Dof::Bend))
        .collect();
    let goals = [Goal::Place {
        bone: end,
        local: tip.grip,
        target: tip.goal,
    }];
    *pose = Solver::new(variables).solve(body, *pose, around, &goals, limits, POLISH_ITERATIONS);
}

fn solve(
    body: &Body,
    pose: &mut HumanPose,
    worlds: &Worlds,
    limb: Limb,
    goal: Vec3,
    turn: Option<Quat>,
) {
    let [upper, lower, end] = limb.chain();
    if !(body.has(upper) && body.has(lower) && body.has(end)) {
        return;
    }
    place(body, pose, worlds, [upper, lower, end], goal);
    if let Some(turn) = turn {
        orient(body, pose, lower, end, turn.normalize());
    }
}

fn fold(
    body: &Body,
    pose: &mut HumanPose,
    [_, lower, end]: [HumanBone; 3],
    grip: Vec3,
    distance: f32,
    range: impl Fn(&Muscle) -> (f32, f32),
) -> Option<Folded> {
    let (axis, bend) = body
        .axis(lower, Dof::Bend)
        .zip(muscle_of(lower, Dof::Bend))?;
    let bend_muscle = muscles()[bend];
    let current = bend_muscle.angle(pose.muscles[bend]);
    let unbent = Quat::from_axis_angle(axis, current).inverse() * body.local_rotation(lower, pose);
    let upper_offset = body.offset(lower);
    let lower_offset = unbent * (body.offset(end) + body.local_rotation(end, pose) * grip);
    let (upper, lower) = (upper_offset.length(), lower_offset.length());
    let reachable = distance.clamp((upper - lower).abs(), upper + lower);
    let angle = bent(
        upper_offset,
        lower_offset,
        axis,
        reachable,
        current,
        range(&bend_muscle),
    );
    pose.muscles[bend] = bend_muscle.value(angle);
    Some(Folded {
        span: upper_offset + Quat::from_axis_angle(axis, angle) * lower_offset,
        upper,
        lower,
    })
}

fn place(
    body: &Body,
    pose: &mut HumanPose,
    worlds: &Worlds,
    [upper, lower, end]: [HumanBone; 3],
    goal: Vec3,
) {
    let root = worlds.position(upper);
    let toward = goal - root;
    let Some(direction) = toward.try_normalize() else {
        return;
    };
    let Some(folded) = fold(
        body,
        pose,
        [upper, lower, end],
        Vec3::ZERO,
        toward.length(),
        |muscle| {
            let (low, high) = muscle.angles(true);
            (low.min(0.0), high.max(0.0))
        },
    ) else {
        return;
    };
    let rotation = worlds.rotation(upper);
    let aimed = arc(rotation * folded.span, direction) * rotation;
    let pole = (worlds.position(lower) - root).normalize_or_zero();
    let desired = elbow(root, goal, folded.upper, folded.lower, pole) - root;
    let (have, want) = match across(pole, direction).length() > FLAT_POLE {
        true => (
            across(aimed * body.offset(lower), direction),
            across(desired, direction),
        ),
        false => (Vec3::ZERO, Vec3::ZERO),
    };
    let angle = match is_short(have) || is_short(want) {
        true => 0.0,
        false => signed_angle(have, want, direction),
    };
    let parent = parent_turn(body, worlds, upper).inverse();
    body.set_local_rotation(
        upper,
        parent * Quat::from_axis_angle(direction, angle) * aimed,
        pose,
    );
}

fn bent(upper: Vec3, lower: Vec3, axis: Vec3, reach: f32, current: f32, limits: (f32, f32)) -> f32 {
    let along = axis * lower.dot(axis);
    let flat = lower - along;
    let normal = axis.cross(flat);
    let cosine_part = upper.dot(flat);
    let sine_part = upper.dot(normal);
    let amplitude = cosine_part.hypot(sine_part);
    if amplitude <= f32::EPSILON {
        return current;
    }
    let need =
        (reach * reach - upper.length_squared() - lower.length_squared()) * 0.5 - upper.dot(along);
    let phase = sine_part.atan2(cosine_part);
    let opening = (need / amplitude).clamp(-1.0, 1.0).acos();
    let wrapped = |angle: f32| {
        (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    };
    let (low, high) = limits;
    let outside = |angle: f32| {
        let beyond = (low - angle).max(angle - high);
        match beyond > LIMIT_SLACK {
            true => beyond,
            false => 0.0,
        }
    };
    let rank = |angle: f32| (outside(angle), (angle - current).abs(), -angle);
    let (one, other) = (wrapped(phase + opening), wrapped(phase - opening));
    let chosen = match rank(one).partial_cmp(&rank(other)) {
        Some(std::cmp::Ordering::Greater) => other,
        _ => one,
    };
    chosen.clamp(low, high)
}

pub(crate) fn orient(
    body: &Body,
    pose: &mut HumanPose,
    lower: HumanBone,
    end: HumanBone,
    turn: Quat,
) {
    if let Some((axis, index)) = body
        .axis(lower, Dof::Twist)
        .zip(muscle_of(lower, Dof::Twist))
    {
        let muscle = muscles()[index];
        let current = muscle.angle(pose.muscles[index]);
        let untwisted =
            body.fk(pose).rotation(lower) * Quat::from_axis_angle(axis, current).inverse();
        let estimate = twist_about(untwisted.inverse() * turn, axis);
        let mut scratch = *pose;
        let mut miss_at = |twist: f32| {
            let carried = untwisted * Quat::from_axis_angle(axis, twist);
            body.set_local_rotation(end, carried.inverse() * turn, &mut scratch);
            (carried * body.local_rotation(end, &scratch)).angle_between(turn)
        };
        let searched = settle(
            &mut miss_at,
            estimate - TWIST_SEARCH,
            estimate + TWIST_SEARCH,
        );
        let current_miss = miss_at(current);
        if miss_at(searched) < current_miss - CONVERGED {
            pose.muscles[index] = muscle.value(searched);
        }
    }
    let carried = body.fk(pose).rotation(lower);
    body.set_local_rotation(end, carried.inverse() * turn, pose);
}

fn settle(miss_at: &mut impl FnMut(f32) -> f32, low: f32, high: f32) -> f32 {
    let ratio = (5f32.sqrt() - 1.0) * 0.5;
    let (mut low, mut high) = (low, high);
    let mut left = high - (high - low) * ratio;
    let mut right = low + (high - low) * ratio;
    let (mut at_left, mut at_right) = (miss_at(left), miss_at(right));
    for _ in 0..TWIST_STEPS {
        if at_left < at_right {
            high = right;
            right = left;
            at_right = at_left;
            left = high - (high - low) * ratio;
            at_left = miss_at(left);
        } else {
            low = left;
            left = right;
            at_left = at_right;
            right = low + (high - low) * ratio;
            at_right = miss_at(right);
        }
    }
    (low + high) * 0.5
}
