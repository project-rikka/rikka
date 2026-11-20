use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::ops::Effect;
use crate::picture::Frame;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const MAX_DETAIL: u32 = 10;
const MIN_SCALE: f32 = 1.0;
const MAX_SCALE: f32 = 100_000.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FractalPush {
    head: PushHeader,
    scale: f32,
    detail: u32,
    seed: u32,
    evolution: f32,
    contrast: f32,
    brightness: f32,
    offset: [f32; 2],
}

const GRAY: u32 = 0;
const COLORED: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FractalNoise {
    scale: f32,
    detail: u32,
    seed: u32,
    evolution: f32,
    contrast: f32,
    brightness: f32,
    offset: (f32, f32),
    colored: bool,
}

impl FractalNoise {
    #[must_use]
    pub fn new(scale: f32) -> FractalNoise {
        FractalNoise {
            scale: scale.clamp(MIN_SCALE, MAX_SCALE),
            detail: 4,
            seed: 0,
            evolution: 0.0,
            contrast: 100.0,
            brightness: 0.0,
            offset: (0.0, 0.0),
            colored: false,
        }
    }

    #[must_use]
    pub fn colored(mut self) -> FractalNoise {
        self.colored = true;
        self
    }

    #[must_use]
    pub fn detail(mut self, octaves: u32) -> FractalNoise {
        self.detail = octaves.clamp(1, MAX_DETAIL);
        self
    }

    #[must_use]
    pub fn seed(mut self, seed: u32) -> FractalNoise {
        self.seed = seed;
        self
    }

    #[must_use]
    pub fn evolution(mut self, evolution: f32) -> FractalNoise {
        self.evolution = match evolution.is_finite() {
            true => evolution,
            false => 0.0,
        };
        self
    }

    #[must_use]
    pub fn contrast(mut self, percent: f32) -> FractalNoise {
        self.contrast = percent.clamp(0.0, 10_000.0);
        self
    }

    #[must_use]
    pub fn brightness(mut self, percent: f32) -> FractalNoise {
        self.brightness = percent.clamp(-200.0, 200.0);
        self
    }

    #[must_use]
    pub fn offset(mut self, x: f32, y: f32) -> FractalNoise {
        self.offset = (x, y);
        self
    }
}

impl Effect for FractalNoise {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let push = FractalPush {
            head: PushHeader::new(&frame.plane, &out)
                .mode(match self.colored {
                    true => COLORED,
                    false => GRAY,
                })
                .clamped(frame.depth.clamps()),
            scale: self.scale,
            detail: self.detail,
            seed: self.seed,
            evolution: self.evolution,
            contrast: self.contrast / 100.0,
            brightness: self.brightness / 100.0,
            offset: [self.offset.0, self.offset.1],
        };
        let cost = f64::from(self.detail) * 4.0;
        work.pass(Shader::Fractal, push, Io::new(&frame.plane, &out), cost)?;
        Ok(frame.with_plane(out))
    }

    fn scaled(&self, factor: f32) -> FractalNoise {
        FractalNoise {
            scale: (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE),
            offset: (self.offset.0 * factor, self.offset.1 * factor),
            ..*self
        }
    }
}

crate::work::push_header!(FractalPush);
