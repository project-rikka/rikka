use rikka_nvml::{Device, NvmlError, Sample, SampleValue, Sampling};

const FIRST_LOOK_MICROS: u64 = 1_000_000;

#[derive(Debug, Default)]
pub(super) struct Interval {
    last_timestamp: Option<u64>,
    last: Option<f32>,
    unsampled: bool,
}

impl Interval {
    pub(super) fn average(&mut self, device: &Device<'_>, sampling: Sampling) -> Option<f32> {
        let fresh = self.fresh(device, sampling)?;
        if !fresh.is_empty() {
            let total: f32 = fresh.iter().map(value_of).sum();
            self.last = Some(share(total / fresh.len() as f32));
        }
        self.last
    }

    fn fresh(&mut self, device: &Device<'_>, sampling: Sampling) -> Option<Vec<Sample>> {
        if self.unsampled {
            return None;
        }
        let mut samples = match device.samples(sampling, self.last_timestamp) {
            Ok(samples) => samples,
            Err(NvmlError::NotSupported) => {
                self.unsampled = true;
                return None;
            }
            Err(_) => return None,
        };
        let Some(latest) = samples.iter().map(|sample| sample.timestamp).max() else {
            return Some(Vec::new());
        };
        let since = self
            .last_timestamp
            .unwrap_or_else(|| latest.saturating_sub(FIRST_LOOK_MICROS));
        self.last_timestamp = Some(latest);
        samples.retain(|sample| sample.timestamp > since);
        Some(samples)
    }
}

pub(super) fn share(percent: f32) -> f32 {
    (percent / 100.0).clamp(0.0, 1.0)
}

fn value_of(sample: &Sample) -> f32 {
    match sample.value {
        SampleValue::U32(value) => value as f32,
        SampleValue::U64(value) => value as f32,
        SampleValue::I64(value) => value as f32,
        SampleValue::F64(value) => value as f32,
    }
}
