use animate_body::{Body, Side, Worlds};
use rikka_model_core::glam::Vec3;

use crate::part::Goal;
use crate::tuning::{CONTACT_DRIFT, CONTACT_FRAMES, CONTACT_HEIGHT, CONTACT_SPEED};

pub(super) struct Footing {
    pub(super) pinned: Vec<bool>,
    pub(super) goals: Vec<Goal>,
}

pub(super) fn footing(body: &Body, worlds: &[Worlds], side: Side) -> Footing {
    let foot = side.foot();
    let scale = body.scale();
    let mut goals: Vec<Goal> = worlds
        .iter()
        .map(|worlds| Goal::from_worlds(worlds, side.leg(), scale))
        .collect();
    let touching: Vec<bool> = worlds
        .iter()
        .enumerate()
        .map(|(at, pinned)| {
            let low = body.sole_height(pinned, side);
            let neighbour = match at {
                0 => worlds.get(1),
                _ => worlds.get(at - 1),
            };
            let slide = neighbour.map_or(0.0, |other| {
                let step: Vec3 = pinned.position(foot) - other.position(foot);
                step.length()
            });
            low < CONTACT_HEIGHT && slide < CONTACT_SPEED
        })
        .collect();
    let mut pinned = vec![false; worlds.len()];
    let mut at = 0;
    while at < touching.len() {
        let anchor = goals[at].position;
        let end = (at..touching.len())
            .find(|later| {
                !touching[*later] || goals[*later].position.distance(anchor) * scale > CONTACT_DRIFT
            })
            .unwrap_or(touching.len());
        if touching[at] && end - at >= CONTACT_FRAMES {
            let middle = goals[(at + end) / 2].rotation;
            let position = goals[at..end]
                .iter()
                .map(|goal| goal.position)
                .sum::<Vec3>()
                / (end - at) as f32;
            for frame in at..end {
                pinned[frame] = true;
                goals[frame] = Goal {
                    position,
                    rotation: middle,
                };
            }
        }
        at = end.max(at + 1);
    }
    Footing { pinned, goals }
}
