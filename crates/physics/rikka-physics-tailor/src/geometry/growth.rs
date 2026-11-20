const STEP_GROWTH: f32 = 2.0;
const STEP_DECAY: f32 = 0.93;
const SLOWEST_GROWTH: f32 = 1.1;
const STEPS: usize = 100;
const FINAL_PASSES: usize = 24;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Growth {
    reached: f32,
}

impl Growth {
    pub(crate) fn reached(&self) -> f32 {
        self.reached
    }

    pub(crate) fn grow(&mut self, start: f32, target: f32, mut pass: impl FnMut(f32) -> usize) {
        let mut radius = self.reached.max(start);
        let mut growth = STEP_GROWTH;
        for _ in 0..STEPS {
            if radius >= target {
                break;
            }
            radius = (radius * growth).min(target);
            growth = (growth * STEP_DECAY).max(SLOWEST_GROWTH);
            pass(radius);
        }
        for _ in 0..FINAL_PASSES {
            if pass(target) == 0 {
                break;
            }
        }
        self.reached = self.reached.max(target);
    }
}

pub(crate) fn followed(redirect: &[u32], mut node: u32) -> u32 {
    while redirect[node as usize] != node {
        node = redirect[node as usize];
    }
    node
}
