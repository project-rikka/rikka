const TICK_EPSILON: f64 = 1.0e-9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Clock {
    rate: f64,
    tick: i64,
    accumulator: f64,
    base: i64,
    planned: u32,
    segment_start: f64,
    segment_end: f64,
    exact_end: bool,
    dropped: f64,
}

impl Clock {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            rate: rate as f64,
            tick: 0,
            accumulator: 0.0,
            base: 0,
            planned: 0,
            segment_start: 0.0,
            segment_end: 0.0,
            exact_end: false,
            dropped: 0.0,
        }
    }

    pub(crate) fn tick(&self) -> i64 {
        self.tick
    }

    pub(crate) fn time(&self) -> f64 {
        self.tick as f64 / self.rate
    }

    pub(crate) fn motion_time(&self) -> f64 {
        self.time() + self.accumulator
    }

    pub(crate) fn alpha(&self) -> f32 {
        (self.accumulator * self.rate).clamp(0.0, 1.0) as f32
    }

    pub(crate) fn dropped(&self) -> f64 {
        self.dropped
    }

    pub(crate) fn restart(&mut self, time: f64) {
        self.tick = (time * self.rate).round() as i64;
        self.accumulator = 0.0;
        self.base = self.tick;
        self.planned = 0;
        self.segment_start = self.time();
        self.segment_end = self.segment_start;
        self.exact_end = true;
    }

    pub(crate) fn plan_frame(&mut self, dt: f64, max_ticks: u32) -> u32 {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let total = self.accumulator + dt;
        let mut ticks = (total * self.rate + TICK_EPSILON).floor() as u32;
        let mut remainder = total - ticks as f64 / self.rate;
        if ticks > max_ticks {
            self.dropped += (ticks - max_ticks) as f64 / self.rate + remainder;
            ticks = max_ticks;
            remainder = 0.0;
        }
        self.plan(ticks, remainder.max(0.0))
    }

    pub(crate) fn plan_until(&mut self, motion_time: f64) -> u32 {
        let target = (motion_time * self.rate + TICK_EPSILON).floor() as i64;
        let ticks = (target - self.tick).max(0) as u32;
        let remainder = (motion_time - (self.tick + ticks as i64) as f64 / self.rate).max(0.0);
        self.plan(ticks, remainder)
    }

    fn plan(&mut self, ticks: u32, remainder: f64) -> u32 {
        self.base = self.tick;
        self.planned = ticks;
        self.segment_end = (self.tick + ticks as i64) as f64 / self.rate + remainder;
        self.exact_end = remainder * self.rate <= TICK_EPSILON;
        self.accumulator = remainder;
        ticks
    }

    pub(crate) fn fraction(&self) -> f32 {
        let step = self.tick - self.base + 1;
        if step == self.planned as i64 && self.exact_end {
            return 1.0;
        }
        let span = self.segment_end - self.segment_start;
        if span <= 0.0 {
            return 1.0;
        }
        let end = (self.tick + 1) as f64 / self.rate;
        ((end - self.segment_start) / span).clamp(0.0, 1.0) as f32
    }

    pub(crate) fn finish_tick(&mut self) {
        self.tick += 1;
    }

    pub(crate) fn finish_frame(&mut self) {
        if self.tick != self.base {
            self.segment_start = self.time();
        }
    }
}
