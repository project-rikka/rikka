use std::time::Duration;

const THRESHOLD_DB: f32 = -48.0;
const HANGOVER: Duration = Duration::from_millis(300);

#[derive(Debug, Default)]
pub(crate) struct Speaking {
    on: bool,
    quiet: Duration,
}

impl Speaking {
    pub(crate) fn is_on(&self) -> bool {
        self.on
    }

    pub(crate) fn update(&mut self, level_db: Option<f32>, frame: Duration) -> Option<bool> {
        if level_db.is_some_and(|level| level >= THRESHOLD_DB) {
            self.quiet = Duration::ZERO;
            if !self.on {
                self.on = true;
                return Some(true);
            }
            return None;
        }
        if !self.on {
            return None;
        }
        self.quiet += frame;
        if self.quiet >= HANGOVER {
            self.on = false;
            return Some(false);
        }
        None
    }
}
