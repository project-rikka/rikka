#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Level {
    None,
    Probable,
    Verified,
}

#[derive(Clone, Debug)]
pub(crate) struct ClockdriftDetector {
    delay_history: [i32; 3],
    level: Level,
    stability_counter: usize,
}

impl ClockdriftDetector {
    pub(crate) fn new() -> ClockdriftDetector {
        ClockdriftDetector {
            delay_history: [0; 3],
            level: Level::None,
            stability_counter: 0,
        }
    }

    pub(crate) fn level(&self) -> Level {
        self.level
    }

    pub(crate) fn update(&mut self, delay_estimate: i32) {
        if delay_estimate == self.delay_history[0] {
            self.stability_counter += 1;
            if self.stability_counter > 7500 {
                self.level = Level::None;
            }
            return;
        }
        self.stability_counter = 0;
        let d1 = self.delay_history[0] - delay_estimate;
        let d2 = self.delay_history[1] - delay_estimate;
        let d3 = self.delay_history[2] - delay_estimate;
        let probable_drift_up = (d1 == -1 && d2 == -2) || (d1 == -2 && d2 == -1);
        let drift_up = probable_drift_up && d3 == -3;
        let probable_drift_down = (d1 == 1 && d2 == 2) || (d1 == 2 && d2 == 1);
        let drift_down = probable_drift_down && d3 == 3;
        if drift_up || drift_down {
            self.level = Level::Verified;
        } else if (probable_drift_up || probable_drift_down) && self.level == Level::None {
            self.level = Level::Probable;
        }
        self.delay_history = [delay_estimate, self.delay_history[0], self.delay_history[1]];
    }
}
