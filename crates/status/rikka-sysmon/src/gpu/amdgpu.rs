use crate::gpu::adapter::Memory;
use crate::gpu::sysfs::{self, Device};

pub(crate) struct Reading {
    pub(crate) usage: f32,
    pub(crate) vram: Option<Memory>,
    pub(crate) gtt: Option<Memory>,
}

pub(crate) fn sample(device: &Device) -> Option<Reading> {
    let busy: f32 = sysfs::read(device.path("gpu_busy_percent"))?;
    Some(Reading {
        usage: (busy / 100.0).clamp(0.0, 1.0),
        vram: pool(device, "vram"),
        gtt: pool(device, "gtt"),
    })
}

fn pool(device: &Device, name: &str) -> Option<Memory> {
    Some(Memory {
        used: sysfs::read(device.path(&format!("mem_info_{name}_used")))?,
        total: sysfs::read(device.path(&format!("mem_info_{name}_total")))?,
    })
}
