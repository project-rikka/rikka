use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::ops::Effect;
use crate::ops::sharpen::UnsharpMask;
use crate::picture::Frame;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const SEARCH: i32 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Distribution {
    Uniform,
    #[default]
    Gaussian,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct NoisePush {
    head: PushHeader,
    amount: f32,
    monochromatic: u32,
    seed: u32,
    pad: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddNoise {
    amount: f32,
    distribution: Distribution,
    monochromatic: bool,
    seed: u32,
}

impl AddNoise {
    #[must_use]
    pub fn new(amount: f32) -> AddNoise {
        AddNoise {
            amount: amount.clamp(0.0, 400.0),
            distribution: Distribution::default(),
            monochromatic: false,
            seed: 0,
        }
    }

    #[must_use]
    pub fn distribution(mut self, distribution: Distribution) -> AddNoise {
        self.distribution = distribution;
        self
    }

    #[must_use]
    pub fn monochromatic(mut self, monochromatic: bool) -> AddNoise {
        self.monochromatic = monochromatic;
        self
    }

    #[must_use]
    pub fn seed(mut self, seed: u32) -> AddNoise {
        self.seed = seed;
        self
    }
}

impl Effect for AddNoise {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let push = NoisePush {
            head: PushHeader::new(&frame.plane, &out)
                .mode(self.distribution as u32)
                .clamped(frame.depth.clamps()),
            amount: self.amount / 100.0 * 0.25,
            monochromatic: u32::from(self.monochromatic),
            seed: self.seed,
            pad: 0,
        };
        work.pass(Shader::Noise, push, Io::new(&frame.plane, &out), 2.0)?;
        Ok(frame.with_plane(out))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DenoisePush {
    head: PushHeader,
    luma: f32,
    color: f32,
    preserve: f32,
    search: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReduceNoise {
    strength: f32,
    preserve_details: f32,
    color: f32,
    sharpen_details: f32,
}

impl ReduceNoise {
    #[must_use]
    pub fn new(strength: f32) -> ReduceNoise {
        ReduceNoise {
            strength: strength.clamp(0.0, 10.0),
            preserve_details: 60.0,
            color: 45.0,
            sharpen_details: 25.0,
        }
    }

    #[must_use]
    pub fn preserve_details(mut self, percent: f32) -> ReduceNoise {
        self.preserve_details = percent.clamp(0.0, 100.0);
        self
    }

    #[must_use]
    pub fn color(mut self, percent: f32) -> ReduceNoise {
        self.color = percent.clamp(0.0, 100.0);
        self
    }

    #[must_use]
    pub fn sharpen_details(mut self, percent: f32) -> ReduceNoise {
        self.sharpen_details = percent.clamp(0.0, 100.0);
        self
    }
}

impl Effect for ReduceNoise {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let push = DenoisePush {
            head: PushHeader::new(&frame.plane, &out).clamped(frame.depth.clamps()),
            luma: self.strength / 10.0 * 0.12,
            color: self.color / 100.0 * 0.25,
            preserve: self.preserve_details / 100.0 * 0.6,
            search: SEARCH,
        };
        let taps = f64::from((2 * SEARCH + 1).pow(2)) * 20.0;
        work.pass(Shader::Denoise, push, Io::new(&frame.plane, &out), taps)?;
        let quiet = frame.with_plane(out);
        match self.sharpen_details > 0.0 {
            true => UnsharpMask::new(self.sharpen_details, 1.0).apply(work, &quiet),
            false => Ok(quiet),
        }
    }
}

crate::work::push_header!(NoisePush, DenoisePush);
