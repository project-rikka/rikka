mod cpu;
mod disk;
mod gpu;
mod memory;
mod network;

use std::rc::Rc;

use rikka_app::prelude::*;

use crate::performance::pane;
use crate::performance::series::Series;
use crate::sampler::Reading;

pub(crate) use cpu::{Cpu, CpuView};
pub(crate) use disk::Disk;
pub(crate) use gpu::Gpu;
pub(crate) use memory::Memory;
pub(crate) use network::Network;

const RATE_FLOOR: f32 = 1_000_000.0;

pub(crate) trait Monitor {
    fn colour(&self) -> Color;

    fn title(&self) -> String;

    fn summary(&self) -> String;

    fn scale(&self) -> f32 {
        1.0
    }

    fn pane(&self) -> Element;
}

#[derive(Clone, Copy)]
pub(crate) struct MonitorData<'a> {
    reading: &'a Reading,
    series: Option<&'a Series>,
}

impl<'a> MonitorData<'a> {
    pub(crate) fn new(reading: &'a Reading, series: Option<&'a Series>) -> Self {
        Self { reading, series }
    }

    pub(crate) fn samples(&self) -> Rc<Vec<f32>> {
        self.series
            .map(|series| series.primary.samples())
            .unwrap_or_default()
    }

    fn secondary(&self) -> Rc<Vec<f32>> {
        self.series
            .map(|series| series.secondary.samples())
            .unwrap_or_default()
    }

    fn ceiling(&self, floor: f32) -> f32 {
        let peak = self
            .series
            .map(|series| series.primary.peak().max(series.secondary.peak()))
            .unwrap_or_default();
        pane::ceiling(peak, floor)
    }
}
