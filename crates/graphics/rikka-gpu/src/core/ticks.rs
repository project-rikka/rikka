use crate::core::device::RenderDevice;

pub(crate) const PASS_TARGET: &str = "rikka_renderer::passes";

#[derive(Clone, Copy, Debug)]
pub struct Ticks {
    mask: u64,
    period_ns: f64,
}

impl Ticks {
    pub fn new(device: &RenderDevice, family: u32) -> Option<Ticks> {
        let valid_bits = unsafe {
            device
                .instance()
                .handle()
                .get_physical_device_queue_family_properties(device.physical())
        }
        .get(family as usize)
        .map_or(0, |properties| properties.timestamp_valid_bits);
        let period_ns = f64::from(device.limits().timestamp_period);
        (valid_bits > 0 && period_ns > 0.0).then_some(Ticks {
            mask: match valid_bits {
                64.. => u64::MAX,
                bits => (1u64 << bits) - 1,
            },
            period_ns,
        })
    }

    pub fn nanos(&self, [start, end]: [u64; 2]) -> f64 {
        (end.wrapping_sub(start) & self.mask) as f64 * self.period_ns
    }

    pub(crate) fn report(&self, pass: &str, frame: u64, stamps: [u64; 2]) {
        tracing::debug!(
            target: PASS_TARGET,
            pass,
            frame,
            micros = self.nanos(stamps) / 1000.0,
            "gpu pass"
        );
    }
}
