use bytemuck::{Pod, Zeroable};
use rikka_gpu::ImageFormat;
use rikka_types::Color;

use crate::error::Result;
use crate::ops::Effect;
use crate::ops::passes::{Domain, gaussian, in_premultiplied};
use crate::picture::Frame;
use crate::plane::{Plane, PlaneRef};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const SHIFT: u32 = 0;
const SILHOUETTE: u32 = 1;
const UNDER: u32 = 2;
const SEED: u32 = 3;
const FLOOD: u32 = 4;
const RING: u32 = 5;

const MAX_DISTANCE: f32 = 2000.0;
const MAX_BLUR: f32 = 1000.0;
const MAX_WIDTH: f32 = 1000.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MattePush {
    head: PushHeader,
    other: PlaneRef,
    k: [f32; 4],
    ink: [f32; 4],
}

fn matte(
    work: &mut Work,
    mode: u32,
    from: &Plane,
    format: ImageFormat,
    k: [f32; 4],
    ink: [f32; 4],
) -> Result<Plane> {
    let out = work.plane(from.extent(), format)?;
    let push = MattePush {
        head: PushHeader::new(from, &out).mode(mode),
        other: from.reference(),
        k,
        ink,
    };
    work.pass(Shader::Matte, push, Io::new(from, &out), 9.0)?;
    Ok(out)
}

fn composite_under(work: &mut Work, top: &Plane, beneath: &Plane) -> Result<Plane> {
    let out = work.plane(top.extent(), top.format())?;
    let push = MattePush {
        head: PushHeader::new(top, &out).mode(UNDER),
        other: beneath.reference(),
        k: [0.0; 4],
        ink: [0.0; 4],
    };
    work.pass(Shader::Matte, push, Io::new(top, &out).read(beneath), 2.0)?;
    Ok(out)
}

fn toward(degrees: f32, distance: f32) -> [f32; 2] {
    let (sin, cos) = degrees.to_radians().sin_cos();
    [cos * distance, -sin * distance]
}

fn ink(frame: &Frame, color: Color, opacity: f32) -> [f32; 4] {
    frame
        .space
        .linear_premultiplied([color.r, color.g, color.b, color.a * opacity])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelShift {
    angle: f32,
    distance: f32,
}

impl ChannelShift {
    #[must_use]
    pub fn new(angle: f32, distance: f32) -> ChannelShift {
        ChannelShift {
            angle,
            distance: distance.clamp(0.0, MAX_DISTANCE),
        }
    }
}

impl Effect for ChannelShift {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        if self.distance <= 0.0 {
            return Ok(frame.clone());
        }
        let [x, y] = toward(self.angle, self.distance);
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            matte(
                work,
                SHIFT,
                &linear,
                linear.format(),
                [x, y, 0.0, 0.0],
                [0.0; 4],
            )
        })
    }

    fn scaled(&self, factor: f32) -> ChannelShift {
        ChannelShift::new(self.angle, self.distance * factor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropShadow {
    color: Color,
    angle: f32,
    distance: f32,
    blur: f32,
    opacity: f32,
}

impl DropShadow {
    #[must_use]
    pub fn new(color: Color) -> DropShadow {
        DropShadow {
            color,
            angle: -45.0,
            distance: 8.0,
            blur: 4.0,
            opacity: 0.75,
        }
    }

    #[must_use]
    pub fn angle(mut self, degrees: f32) -> DropShadow {
        self.angle = degrees;
        self
    }

    #[must_use]
    pub fn distance(mut self, distance: f32) -> DropShadow {
        self.distance = distance.clamp(0.0, MAX_DISTANCE);
        self
    }

    #[must_use]
    pub fn blur(mut self, blur: f32) -> DropShadow {
        self.blur = blur.clamp(0.0, MAX_BLUR);
        self
    }

    #[must_use]
    pub fn opacity(mut self, opacity: f32) -> DropShadow {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

impl Effect for DropShadow {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let ink = ink(frame, self.color, self.opacity);
        if ink[3] <= 0.0 {
            return Ok(frame.clone());
        }
        let [x, y] = toward(self.angle, self.distance);
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            let cast = matte(
                work,
                SILHOUETTE,
                &linear,
                linear.format(),
                [x, y, 0.0, 0.0],
                ink,
            )?;
            let shadow = gaussian(work, &cast, self.blur)?;
            composite_under(work, &linear, &shadow)
        })
    }

    fn scaled(&self, factor: f32) -> DropShadow {
        DropShadow {
            distance: self.distance * factor,
            blur: self.blur * factor,
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    color: Color,
    width: f32,
    softness: f32,
}

impl Outline {
    #[must_use]
    pub fn new(color: Color) -> Outline {
        Outline {
            color,
            width: 4.0,
            softness: 0.0,
        }
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Outline {
        self.width = width.clamp(0.0, MAX_WIDTH);
        self
    }

    #[must_use]
    pub fn softness(mut self, softness: f32) -> Outline {
        self.softness = softness.clamp(0.0, MAX_WIDTH);
        self
    }
}

impl Effect for Outline {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let ink = ink(frame, self.color, 1.0);
        if ink[3] <= 0.0 || self.width + self.softness * 0.5 <= 0.0 {
            return Ok(frame.clone());
        }
        let reach = (self.width + self.softness * 0.5 + 1.0).ceil();
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            let mut seeds = matte(
                work,
                SEED,
                &linear,
                ImageFormat::Rgba32Float,
                [0.0; 4],
                [0.0; 4],
            )?;
            for step in jumps(reach) {
                seeds = matte(
                    work,
                    FLOOD,
                    &seeds,
                    ImageFormat::Rgba32Float,
                    [step as f32, 0.0, 0.0, 0.0],
                    [0.0; 4],
                )?;
            }
            let ring = matte(
                work,
                RING,
                &seeds,
                linear.format(),
                [self.width, self.softness, 0.0, 0.0],
                ink,
            )?;
            composite_under(work, &linear, &ring)
        })
    }

    fn scaled(&self, factor: f32) -> Outline {
        Outline {
            width: self.width * factor,
            softness: self.softness * factor,
            ..*self
        }
    }
}

fn jumps(reach: f32) -> Vec<u32> {
    let reach = reach.max(1.0) as u32;
    let mut step = 1u32 << (31 - reach.leading_zeros());
    let mut steps = Vec::new();
    while step >= 1 {
        steps.push(step);
        step /= 2;
    }
    steps.push(1);
    steps
}

crate::work::push_header!(MattePush);
