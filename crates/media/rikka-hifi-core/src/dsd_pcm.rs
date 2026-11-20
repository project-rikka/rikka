use std::f64::consts::PI;

const HIGHEST_PCM_RATE: u32 = 352_800;
const SMALLEST_DECIMATION: u32 = 32;
const TAPS_PER_DECIMATION: usize = 24;

#[derive(Debug, Clone)]
pub struct DsdToPcm {
    channels: usize,
    rate: u32,
    stride: usize,
    tables: Vec<[f32; 256]>,
    history: Vec<u8>,
    newest: usize,
    phase: usize,
}

impl DsdToPcm {
    #[must_use]
    pub fn pcm_rate(dsd_rate: u32) -> u32 {
        let mut rate = dsd_rate / SMALLEST_DECIMATION;
        while rate > HIGHEST_PCM_RATE {
            rate /= 2;
        }
        rate.max(1)
    }

    #[must_use]
    pub fn new(dsd_rate: u32, channels: u16) -> DsdToPcm {
        let rate = Self::pcm_rate(dsd_rate);
        let decimation = (dsd_rate / rate).max(8) as usize;
        let stride = decimation / 8;
        let taps = decimation * TAPS_PER_DECIMATION;
        let tables = tables(&lowpass(taps, 0.25 / decimation as f64));
        let channels = usize::from(channels.max(1));
        DsdToPcm {
            channels,
            rate,
            stride,
            history: vec![0x69; tables.len() * channels],
            tables,
            newest: 0,
            phase: 0,
        }
    }

    #[must_use]
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn process(&mut self, dsd: &[u8], out: &mut Vec<f32>) {
        let depth = self.tables.len();
        for step in dsd.chunks_exact(self.channels) {
            self.newest = (self.newest + 1) % depth;
            let row = self.newest * self.channels;
            self.history[row..row + self.channels].copy_from_slice(step);
            self.phase += 1;
            if self.phase < self.stride {
                continue;
            }
            self.phase = 0;
            for channel in 0..self.channels {
                out.push(self.sample(channel));
            }
        }
    }

    pub fn reset(&mut self) {
        self.history.fill(0x69);
        self.newest = 0;
        self.phase = 0;
    }

    fn sample(&self, channel: usize) -> f32 {
        let depth = self.tables.len();
        let mut sum = 0.0;
        for (age, table) in self.tables.iter().enumerate() {
            let at = (self.newest + depth - age) % depth;
            sum += table[usize::from(self.history[at * self.channels + channel])];
        }
        sum
    }
}

fn lowpass(taps: usize, cutoff: f64) -> Vec<f64> {
    let middle = (taps - 1) as f64 / 2.0;
    let mut filter: Vec<f64> = (0..taps)
        .map(|n| {
            let offset = n as f64 - middle;
            let ideal = if offset == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * PI * cutoff * offset).sin() / (PI * offset)
            };
            let position = n as f64 / (taps - 1) as f64;
            let window =
                0.42 - 0.5 * (2.0 * PI * position).cos() + 0.08 * (4.0 * PI * position).cos();
            ideal * window
        })
        .collect();
    let total: f64 = filter.iter().sum();
    for tap in &mut filter {
        *tap /= total;
    }
    filter
}

fn tables(filter: &[f64]) -> Vec<[f32; 256]> {
    filter
        .as_chunks::<8>()
        .0
        .iter()
        .map(|taps| {
            std::array::from_fn(|byte| {
                taps.iter()
                    .enumerate()
                    .map(
                        |(age, tap)| {
                            if (byte >> age) & 1 == 1 { *tap } else { -*tap }
                        },
                    )
                    .sum::<f64>() as f32
            })
        })
        .collect()
}
