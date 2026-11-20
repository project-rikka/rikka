#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    pub rate: u32,
    pub quantum: u32,
    pub min_quantum: u32,
    pub max_quantum: u32,
    pub forced_rate: u32,
    pub forced_quantum: u32,
    pub allowed_rates: Vec<u32>,
}

impl Graph {
    pub fn running_rate(&self) -> u32 {
        match self.forced_rate {
            0 => self.rate,
            forced => forced,
        }
    }

    pub fn running_quantum(&self) -> u32 {
        match self.forced_quantum {
            0 => self.quantum,
            forced => forced,
        }
    }

    pub fn latency(&self) -> Option<f64> {
        crate::format::latency(self.running_quantum(), self.running_rate())
    }
}
