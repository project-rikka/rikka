pub const MAX_BURSTS: usize = 16;
pub const MAX_BURST_COUNT: u32 = (1 << 24) - 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    pub time: f32,
    pub count: u32,
    pub duration: f32,
}

impl Burst {
    pub fn new(time: f32, count: u32) -> Self {
        Self {
            time,
            count,
            duration: 0.0,
        }
    }

    pub fn over(self, duration: f32) -> Self {
        Self { duration, ..self }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Emission {
    pub rate: f32,
    pub bursts: Vec<Burst>,
    pub start: f32,
    pub end: f32,
    pub lifetime: f32,
    pub lifetime_variance: f32,
    pub max_particles: u32,
}

impl Default for Emission {
    fn default() -> Self {
        Self {
            rate: 20.0,
            bursts: Vec::new(),
            start: 0.0,
            end: f32::INFINITY,
            lifetime: 2.0,
            lifetime_variance: 0.0,
            max_particles: 65_536,
        }
    }
}

impl Emission {
    pub fn slots(&self) -> u32 {
        let rate = self.rate;
        let lifetime = self.lifetime;
        if !(rate > 0.0 && rate.is_finite() && lifetime > 0.0 && lifetime.is_finite()) {
            return 0;
        }
        let living = (rate * lifetime).ceil() + 1.0;
        let emitted = match self.end.is_finite() {
            true => ((self.end - self.start).max(0.0) * rate).ceil() + 1.0,
            false => living,
        };
        let slots = living.min(emitted).min(self.max_particles as f32);
        slots.max(0.0) as u32
    }

    pub fn burst_particles(&self) -> u32 {
        self.bursts
            .iter()
            .take(MAX_BURSTS)
            .map(|burst| burst.count.min(MAX_BURST_COUNT))
            .sum()
    }
}
