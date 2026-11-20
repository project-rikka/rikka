use crate::common::{BINS, LONG_STARTUP_BLOCKS, Spectrum};
use crate::fast_math::{exp, log};

const SIMULT: usize = 3;

pub(crate) struct QuantileNoiseEstimator {
    density: [f32; SIMULT * BINS],
    log_quantile: [f32; SIMULT * BINS],
    quantile: Spectrum,
    counter: [i32; SIMULT],
    num_updates: i32,
}

impl QuantileNoiseEstimator {
    pub(crate) fn new() -> QuantileNoiseEstimator {
        const ONE_BY_SIMULT: f32 = 1.0 / SIMULT as f32;
        let counter = std::array::from_fn(|i| {
            (LONG_STARTUP_BLOCKS as f32 * (i as f32 + 1.0) * ONE_BY_SIMULT).floor() as i32
        });
        QuantileNoiseEstimator {
            density: [0.3; SIMULT * BINS],
            log_quantile: [8.0; SIMULT * BINS],
            quantile: [0.0; BINS],
            counter,
            num_updates: 1,
        }
    }

    pub(crate) fn estimate(&mut self, signal_spectrum: &Spectrum, noise_spectrum: &mut Spectrum) {
        const WIDTH: f32 = 0.01;
        const ONE_BY_WIDTH_PLUS_2: f32 = 1.0 / (2.0 * WIDTH);
        let log_spectrum = signal_spectrum.map(log);

        let mut quantile_to_return = None;
        for s in 0..SIMULT {
            let k = s * BINS;
            let counter = self.counter[s] as f32;
            let one_by_counter_plus_1 = 1.0 / (counter + 1.0);
            let log_quantiles = &mut self.log_quantile[k..k + BINS];
            let densities = &mut self.density[k..k + BINS];
            for ((&log_signal, log_quantile), density) in
                log_spectrum.iter().zip(log_quantiles).zip(densities)
            {
                let delta = if *density > 1.0 {
                    40.0 / *density
                } else {
                    40.0
                };
                let multiplier = delta * one_by_counter_plus_1;
                if log_signal > *log_quantile {
                    *log_quantile += 0.25 * multiplier;
                } else {
                    *log_quantile -= 0.75 * multiplier;
                }
                if (log_signal - *log_quantile).abs() < WIDTH {
                    *density = (counter * *density + ONE_BY_WIDTH_PLUS_2) * one_by_counter_plus_1;
                }
            }

            if self.counter[s] >= LONG_STARTUP_BLOCKS {
                self.counter[s] = 0;
                if self.num_updates >= LONG_STARTUP_BLOCKS {
                    quantile_to_return = Some(k);
                }
            }
            self.counter[s] += 1;
        }

        if self.num_updates < LONG_STARTUP_BLOCKS {
            quantile_to_return = Some(BINS * (SIMULT - 1));
            self.num_updates += 1;
        }

        if let Some(start) = quantile_to_return {
            for (quantile, &log_quantile) in self
                .quantile
                .iter_mut()
                .zip(&self.log_quantile[start..start + BINS])
            {
                *quantile = exp(log_quantile);
            }
        }

        *noise_spectrum = self.quantile;
    }
}
