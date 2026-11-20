const STEP_ROUNDING: f64 = 0.05;
const STEPS_PER_SECOND: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StepRate {
    per_second: u32,
}

impl StepRate {
    #[must_use]
    pub const fn new(steps_per_second: u32) -> StepRate {
        StepRate {
            per_second: if steps_per_second == 0 {
                1
            } else {
                steps_per_second
            },
        }
    }

    #[must_use]
    pub fn step_of(self, time: f32) -> u32 {
        (f64::from(time) * f64::from(self.per_second) + STEP_ROUNDING)
            .floor()
            .max(0.0) as u32
    }

    #[must_use]
    pub fn time_of(self, step: u32) -> f64 {
        f64::from(step) / f64::from(self.per_second)
    }

    #[must_use]
    pub fn per_second(self) -> u32 {
        self.per_second
    }
}

impl Default for StepRate {
    fn default() -> StepRate {
        StepRate::new(STEPS_PER_SECOND)
    }
}
