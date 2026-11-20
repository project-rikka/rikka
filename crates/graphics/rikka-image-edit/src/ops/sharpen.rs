use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::ops::Effect;
use crate::ops::blur::{LensBlur, MotionBlur};
use crate::ops::noise::ReduceNoise;
use crate::ops::passes::{Domain, arith, gaussian, premultiplied, straightened};
use crate::picture::Frame;
use crate::plane::{Plane, PlaneRef};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const UNSHARP: u32 = 0;
const HIGH_PASS: u32 = 1;
const CLARITY: u32 = 2;

const ITERATIONS: usize = 10;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SharpenPush {
    head: PushHeader,
    blurred: PlaneRef,
    amount: f32,
    threshold: f32,
    pad: [f32; 2],
}

pub(crate) fn sharpened(
    work: &mut Work,
    frame: &Frame,
    radius: f32,
    mode: u32,
    amount: f32,
    threshold: f32,
) -> Result<Frame> {
    let encoded = premultiplied(work, frame, Domain::Encoded)?;
    let blurred = gaussian(work, &encoded, radius)?;
    let out = work.plane(frame.extent(), frame.depth.working())?;
    let push = SharpenPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(mode)
            .clamped(frame.depth.clamps()),
        blurred: blurred.reference(),
        amount,
        threshold,
        pad: [0.0; 2],
    };
    work.pass(
        Shader::Sharpen,
        push,
        Io::new(&frame.plane, &out).read(&blurred),
        2.0,
    )?;
    Ok(frame.with_plane(out))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnsharpMask {
    amount: f32,
    radius: f32,
    threshold: f32,
}

impl UnsharpMask {
    #[must_use]
    pub fn new(amount: f32, radius: f32) -> UnsharpMask {
        UnsharpMask {
            amount: amount.clamp(1.0, 500.0),
            radius: radius.clamp(0.1, 1000.0),
            threshold: 0.0,
        }
    }

    #[must_use]
    pub fn threshold(mut self, levels: f32) -> UnsharpMask {
        self.threshold = levels.clamp(0.0, 255.0);
        self
    }
}

impl Effect for UnsharpMask {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        sharpened(
            work,
            frame,
            self.radius,
            UNSHARP,
            self.amount / 100.0,
            self.threshold / 255.0,
        )
    }

    fn scaled(&self, factor: f32) -> UnsharpMask {
        UnsharpMask {
            radius: (self.radius * factor).max(0.1),
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HighPass {
    radius: f32,
}

impl HighPass {
    #[must_use]
    pub fn new(radius: f32) -> HighPass {
        HighPass {
            radius: radius.clamp(0.1, 1000.0),
        }
    }
}

impl Effect for HighPass {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        sharpened(work, frame, self.radius, HIGH_PASS, 1.0, 0.0)
    }

    fn scaled(&self, factor: f32) -> HighPass {
        HighPass::new((self.radius * factor).max(0.1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clarity {
    amount: f32,
    radius: f32,
}

impl Clarity {
    #[must_use]
    pub fn new(amount: f32) -> Clarity {
        Clarity {
            amount: amount.clamp(-100.0, 100.0),
            radius: 50.0,
        }
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Clarity {
        self.radius = radius.clamp(1.0, 500.0);
        self
    }
}

impl Effect for Clarity {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        sharpened(
            work,
            frame,
            self.radius,
            CLARITY,
            self.amount / 100.0 * 1.5,
            0.0,
        )
    }

    fn scaled(&self, factor: f32) -> Clarity {
        Clarity {
            radius: (self.radius * factor).max(1.0),
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Removal {
    GaussianBlur,
    LensBlur,
    MotionBlur { angle: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmartSharpen {
    amount: f32,
    radius: f32,
    reduce_noise: f32,
    removal: Removal,
}

impl SmartSharpen {
    #[must_use]
    pub fn new(amount: f32, radius: f32) -> SmartSharpen {
        SmartSharpen {
            amount: amount.clamp(1.0, 500.0),
            radius: radius.clamp(0.1, 64.0),
            reduce_noise: 10.0,
            removal: Removal::LensBlur,
        }
    }

    #[must_use]
    pub fn reduce_noise(mut self, percent: f32) -> SmartSharpen {
        self.reduce_noise = percent.clamp(0.0, 100.0);
        self
    }

    #[must_use]
    pub fn removal(mut self, removal: Removal) -> SmartSharpen {
        self.removal = removal;
        self
    }

    fn spread(&self, work: &mut Work, plane: &Plane) -> Result<Plane> {
        match self.removal {
            Removal::GaussianBlur => gaussian(work, plane, self.radius),
            Removal::LensBlur => LensBlur::new(self.radius).blur_premultiplied(work, plane),
            Removal::MotionBlur { angle } => {
                MotionBlur::new(angle, self.radius * 2.0).blur_premultiplied(work, plane)
            }
        }
    }
}

impl Effect for SmartSharpen {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let original = premultiplied(work, frame, Domain::Linear)?;
        let source = match self.reduce_noise > 0.0 {
            true => {
                let denoised = ReduceNoise::new(self.reduce_noise / 20.0).apply(work, frame)?;
                premultiplied(work, &denoised, Domain::Linear)?
            }
            false => original.clone(),
        };
        let mut estimate = source.clone();
        for _ in 0..ITERATIONS {
            let spread = self.spread(work, &estimate)?;
            let ratio = arith(
                work,
                &source,
                &spread,
                arith::RATIO,
                [1e-4, 0.0, 0.0, 0.0],
                [0.0; 4],
            )?;
            let correction = self.spread(work, &ratio)?;
            estimate = arith(
                work,
                &estimate,
                &correction,
                arith::MULTIPLY,
                [0.0; 4],
                [0.0; 4],
            )?;
        }
        let mixed = arith(
            work,
            &original,
            &estimate,
            arith::MIX,
            [self.amount / 100.0, 0.0, 0.0, 0.0],
            [0.0; 4],
        )?;
        Ok(frame.with_plane(straightened(work, frame, &mixed, Domain::Linear)?))
    }

    fn scaled(&self, factor: f32) -> SmartSharpen {
        SmartSharpen {
            radius: (self.radius * factor).max(0.1),
            ..*self
        }
    }
}

crate::work::push_header!(SharpenPush);
