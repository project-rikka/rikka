use std::time::Duration;

use crate::level::SILENT_DB;

const PERIOD: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub(crate) struct Meter {
    frames: u32,
    counted: u32,
    loudest: f32,
}

impl Meter {
    pub(crate) fn new(frame: Duration) -> Meter {
        let frames = (PERIOD.as_secs_f64() / frame.as_secs_f64().max(f64::EPSILON)) as u32;
        Meter {
            frames: frames.max(1),
            counted: 0,
            loudest: SILENT_DB,
        }
    }

    pub(crate) fn add(&mut self, level_db: f32) -> Option<f32> {
        self.loudest = self.loudest.max(level_db);
        self.counted += 1;
        if self.counted < self.frames {
            return None;
        }
        let loudest = self.loudest;
        self.counted = 0;
        self.loudest = SILENT_DB;
        Some(loudest)
    }
}
