use animate_body::Side;
use animate_motion::PoseSample;

const SPREAD: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    Curl,
    Spread,
    Finger(usize),
}

pub fn grip_of(pose_sample: &PoseSample, side: Side, grip: Grip) -> f32 {
    let muscles = grip_muscles(side, grip);
    let total: f32 = muscles.iter().map(|at| pose_sample.pose.muscles[*at]).sum();
    total / muscles.len().max(1) as f32
}

pub fn grip_muscles(side: Side, grip: Grip) -> Vec<usize> {
    let fingers = side.fingers();
    let start = fingers.start;
    match grip {
        Grip::Curl => fingers.filter(|at| (at - start) % 4 != SPREAD).collect(),
        Grip::Spread => fingers.filter(|at| (at - start) % 4 == SPREAD).collect(),
        Grip::Finger(finger) => (0..4)
            .filter(|at| *at != SPREAD)
            .map(|at| start + finger * 4 + at)
            .collect(),
    }
}
