use std::time::Instant;

#[derive(Debug)]
pub struct Clock {
    started: Instant,
    moment: u32,
}

impl Clock {
    #[must_use]
    pub fn new() -> Clock {
        Clock {
            started: Instant::now(),
            moment: 0,
        }
    }

    pub fn now(&mut self) -> u32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "an input device's clock is what wayland's own is: milliseconds that wrap"
        )]
        let elapsed = self.started.elapsed().as_millis() as u32;
        self.moment = elapsed.max(self.moment.wrapping_add(1));
        self.moment
    }
}

impl Default for Clock {
    fn default() -> Clock {
        Clock::new()
    }
}
