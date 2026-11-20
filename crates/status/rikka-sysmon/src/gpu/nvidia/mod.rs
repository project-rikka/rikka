mod interval;
mod recorder;

use std::collections::HashMap;
use std::sync::Arc;

use rikka_nvml::{Nvml, Sampling, TemperatureSensor};

use crate::gpu::adapter::Memory;
use interval::{Interval, share};
use recorder::Recorder;

pub(crate) struct Reading {
    pub(crate) usage: f32,
    pub(crate) encode: Option<f32>,
    pub(crate) decode: Option<f32>,
    pub(crate) memory: Option<Memory>,
    pub(crate) temperature: Option<f32>,
    pub(crate) power: Option<f32>,
}

#[derive(Debug, Default)]
struct Intervals {
    usage: Interval,
    encode: Interval,
    decode: Interval,
}

#[derive(Debug)]
pub(crate) struct Nvidia {
    nvml: Arc<Nvml>,
    intervals: HashMap<u32, Intervals>,
    recorder: Option<Recorder>,
}

impl Nvidia {
    pub(crate) fn open() -> Option<Self> {
        let nvml = rikka_nvml::load().ok()?;
        Some(Self {
            nvml: Arc::new(nvml),
            intervals: HashMap::new(),
            recorder: None,
        })
    }

    pub(crate) fn name(&self, index: u32) -> Option<String> {
        self.nvml.device_by_index(index).ok()?.name().ok()
    }

    pub(crate) fn sample(&mut self, index: u32) -> Option<Reading> {
        let device = self.nvml.device_by_index(index).ok()?;
        let intervals = self.intervals.entry(index).or_default();
        let usage = intervals
            .usage
            .average(&device, Sampling::GpuUtilization)
            .or_else(|| Some(share(device.utilization_rates().ok()?.gpu as f32)))?;
        Some(Reading {
            usage,
            encode: intervals
                .encode
                .average(&device, Sampling::EncoderUtilization)
                .or_else(|| Some(share(device.encoder_utilization().ok()?.utilization as f32))),
            decode: intervals
                .decode
                .average(&device, Sampling::DecoderUtilization)
                .or_else(|| Some(share(device.decoder_utilization().ok()?.utilization as f32))),
            memory: device.memory_info().ok().map(|memory| Memory {
                used: memory.used,
                total: memory.total,
            }),
            temperature: device
                .temperature(TemperatureSensor::Gpu)
                .ok()
                .map(|degrees| degrees as f32),
            power: device
                .power_usage()
                .ok()
                .map(|milliwatts| milliwatts as f32 / 1000.0),
        })
    }

    pub(crate) fn clients(&mut self) -> HashMap<i32, f32> {
        let recorder = self
            .recorder
            .get_or_insert_with(|| Recorder::start(Arc::clone(&self.nvml)));
        let mut clients: HashMap<i32, f32> = HashMap::new();
        for index in 0..self.nvml.device_count().unwrap_or(0) {
            for (pid, share) in recorder.take(index) {
                let usage = clients.entry(pid as i32).or_default();
                *usage = usage.max(share);
            }
        }
        clients
    }

    pub(crate) fn index_of(&self, slot: &str) -> Option<u32> {
        let count = self.nvml.device_count().ok()?;
        (0..count).find(|index| {
            self.nvml
                .device_by_index(*index)
                .ok()
                .and_then(|device| device.pci_info().ok())
                .is_some_and(|pci| {
                    let bus_id = pci.bus_id.to_ascii_lowercase();
                    bus_id.ends_with(&slot.to_ascii_lowercase())
                })
        })
    }
}
