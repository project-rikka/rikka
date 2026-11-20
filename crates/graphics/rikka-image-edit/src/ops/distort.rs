use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::ops::Effect;
use crate::ops::passes::{Domain, in_premultiplied};
use crate::picture::{Frame, Picture};
use crate::plane::{Plane, PlaneRef};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const DISPLACE: u32 = 0;
const LENS: u32 = 1;

const MAX_DISPLACEMENT: f32 = 4000.0;
const MAX_LENS: f32 = 100.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DistortPush {
    head: PushHeader,
    other: PlaneRef,
    k: [f32; 4],
}

fn distorted(
    work: &mut Work,
    mode: u32,
    linear: &Plane,
    other: Option<&Plane>,
    k: [f32; 4],
) -> Result<Plane> {
    let out = work.plane(linear.extent(), linear.format())?;
    let push = DistortPush {
        head: PushHeader::new(linear, &out).mode(mode),
        other: other.unwrap_or(linear).reference(),
        k,
    };
    let io = match other {
        Some(map) => Io::new(linear, &out).read(map),
        None => Io::new(linear, &out),
    };
    work.pass(Shader::Distort, push, io, 6.0)?;
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Displace {
    map: Picture,
    horizontal: f32,
    vertical: f32,
}

impl Displace {
    #[must_use]
    pub fn new(map: Picture, horizontal: f32, vertical: f32) -> Displace {
        let limited = |amount: f32| amount.clamp(-MAX_DISPLACEMENT, MAX_DISPLACEMENT);
        Displace {
            map,
            horizontal: limited(horizontal),
            vertical: limited(vertical),
        }
    }
}

impl Effect for Displace {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        if self.horizontal == 0.0 && self.vertical == 0.0 {
            return Ok(frame.clone());
        }
        let map = &self.map.frame().plane;
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            distorted(
                work,
                DISPLACE,
                &linear,
                Some(map),
                [self.horizontal, self.vertical, 0.0, 0.0],
            )
        })
    }

    fn scaled(&self, factor: f32) -> Displace {
        Displace {
            horizontal: self.horizontal * factor,
            vertical: self.vertical * factor,
            ..self.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensDistortion {
    amount: f32,
    center: (f32, f32),
}

impl LensDistortion {
    #[must_use]
    pub fn new(amount: f32) -> LensDistortion {
        LensDistortion {
            amount: amount.clamp(-MAX_LENS, MAX_LENS),
            center: (0.5, 0.5),
        }
    }

    #[must_use]
    pub fn center(mut self, x: f32, y: f32) -> LensDistortion {
        self.center = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        self
    }
}

impl Effect for LensDistortion {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        if self.amount == 0.0 {
            return Ok(frame.clone());
        }
        let [width, height] = frame.extent();
        let center = [self.center.0 * width as f32, self.center.1 * height as f32];
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            distorted(
                work,
                LENS,
                &linear,
                None,
                [center[0], center[1], self.amount / MAX_LENS, 0.0],
            )
        })
    }
}

crate::work::push_header!(DistortPush);
