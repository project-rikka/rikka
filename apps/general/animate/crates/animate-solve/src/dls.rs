use animate_body::{Body, HumanBone, HumanPose, muscles};

use crate::cholesky::solve;
use crate::effector::Goal;
use crate::tuning::{DAMPING, PULL, STEP, STIFF_HEAD, STIFF_LIMB, STIFF_SHOULDER, STIFF_TORSO};

#[derive(Clone, Debug)]
pub(crate) struct Solver {
    variables: Vec<usize>,
    stiffness: Vec<f64>,
}

fn stiffness(index: usize) -> f64 {
    match muscles()[index].bone {
        HumanBone::Spine | HumanBone::Chest | HumanBone::UpperChest => STIFF_TORSO,
        HumanBone::Neck | HumanBone::Head => STIFF_HEAD,
        HumanBone::LeftShoulder | HumanBone::RightShoulder => STIFF_SHOULDER,
        _ => STIFF_LIMB,
    }
}

impl Solver {
    pub(crate) fn new(mut variables: Vec<usize>) -> Self {
        variables.sort_unstable();
        variables.dedup();
        let stiffness = variables.iter().copied().map(stiffness).collect();
        Self {
            variables,
            stiffness,
        }
    }

    fn size(&self) -> usize {
        self.variables.len()
    }

    pub(crate) fn carry(&self, from: &HumanPose, onto: &mut HumanPose) {
        for index in &self.variables {
            onto.muscles[*index] = from.muscles[*index];
        }
    }

    fn nudged(&self, pose: &HumanPose, variable: usize, amount: f32) -> HumanPose {
        let mut nudged = *pose;
        nudged.muscles[self.variables[variable]] += amount;
        nudged
    }

    fn advanced(&self, pose: &HumanPose, step: &[f64]) -> HumanPose {
        let mut nudged = *pose;
        for (index, amount) in self.variables.iter().zip(step) {
            nudged.muscles[*index] += *amount as f32;
        }
        nudged
    }

    fn apart(&self, pose: &HumanPose, start: &HumanPose) -> Vec<f64> {
        self.variables
            .iter()
            .map(|index| f64::from(pose.muscles[*index] - start.muscles[*index]))
            .collect()
    }

    fn residuals(body: &Body, pose: &HumanPose, goals: &[Goal]) -> Vec<f64> {
        let worlds = body.fk(pose);
        goals
            .iter()
            .flat_map(|goal| goal.residual(&worlds).to_array())
            .map(f64::from)
            .collect()
    }

    pub(crate) fn solve(
        &self,
        body: &Body,
        mut pose: HumanPose,
        start: &HumanPose,
        goals: &[Goal],
        limits: bool,
        iterations: usize,
    ) -> HumanPose {
        let size = self.size();
        if size == 0 {
            return pose;
        }
        for _ in 0..iterations {
            let here = Self::residuals(body, &pose, goals);
            let rows = here.len();
            let mut jacobian = vec![0.0; rows * size];
            for variable in 0..size {
                let moved = Self::residuals(body, &self.nudged(&pose, variable, STEP), goals);
                for row in 0..rows {
                    jacobian[row * size + variable] = (moved[row] - here[row]) / f64::from(STEP);
                }
            }
            let mut normal = vec![0.0; size * size];
            for row in 0..rows {
                let line = &jacobian[row * size..(row + 1) * size];
                for one in 0..size {
                    if line[one] == 0.0 {
                        continue;
                    }
                    for other in 0..=one {
                        normal[one * size + other] += line[one] * line[other];
                    }
                }
            }
            for one in 0..size {
                for other in 0..one {
                    normal[other * size + one] = normal[one * size + other];
                }
                normal[one * size + one] += (DAMPING + PULL) * self.stiffness[one];
            }
            let apart = self.apart(&pose, start);
            let gradient: Vec<f64> = (0..size)
                .map(|variable| {
                    let pulled: f64 = (0..rows)
                        .map(|row| jacobian[row * size + variable] * here[row])
                        .sum();
                    -pulled - PULL * self.stiffness[variable] * apart[variable]
                })
                .collect();
            let Some(step) = self.bounded(&normal, &gradient, &pose, start, limits) else {
                break;
            };
            pose = self.advanced(&pose, &step);
            body.clamp_around(&mut pose, start, limits);
        }
        pose
    }

    fn bounded(
        &self,
        normal: &[f64],
        gradient: &[f64],
        pose: &HumanPose,
        start: &HumanPose,
        limits: bool,
    ) -> Option<Vec<f64>> {
        let size = self.size();
        let mut clamped = vec![false; size];
        let mut system = vec![0.0; normal.len()];
        let mut step = vec![0.0; gradient.len()];
        loop {
            system.copy_from_slice(normal);
            step.copy_from_slice(gradient);
            for variable in (0..size).filter(|variable| clamped[*variable]) {
                for other in 0..size {
                    system[variable * size + other] = 0.0;
                    system[other * size + variable] = 0.0;
                }
                system[variable * size + variable] = 1.0;
                step[variable] = 0.0;
            }
            if !solve(&mut system, size, &mut step) {
                return None;
            }
            let mut pressing = false;
            for (at, index) in self.variables.iter().copied().enumerate() {
                let value = pose.muscles[index];
                let (low, high) = muscles()[index].bounds_around(limits, start.muscles[index]);
                let pressing_low = value <= low && step[at] < 0.0;
                let pressing_high = value >= high && step[at] > 0.0;
                if !clamped[at] && (pressing_low || pressing_high) {
                    clamped[at] = true;
                    pressing = true;
                }
            }
            if !pressing {
                return Some(step);
            }
        }
    }
}
