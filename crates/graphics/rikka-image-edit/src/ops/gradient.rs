use bytemuck::{Pod, Zeroable};
use rikka_types::{Color, Point};

use crate::error::{Error, Result};
use crate::ops::Effect;
use crate::picture::{Depth, Frame};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const INK_COLORS: u32 = 0;
const INK_FADE: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GradientShape {
    #[default]
    Linear,
    Reflected,
    Radial,
    Diamond,
    Conical,
}

impl GradientShape {
    pub const ALL: [GradientShape; 5] = [
        GradientShape::Linear,
        GradientShape::Reflected,
        GradientShape::Radial,
        GradientShape::Diamond,
        GradientShape::Conical,
    ];

    fn code(self) -> u32 {
        match self {
            GradientShape::Linear => 0,
            GradientShape::Reflected => 1,
            GradientShape::Radial => 2,
            GradientShape::Diamond => 3,
            GradientShape::Conical => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientInk {
    Colors { from: Color, to: Color },
    Fade { from: f32, to: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    shape: GradientShape,
    from: Point,
    to: Point,
    ink: GradientInk,
}

impl Gradient {
    #[must_use]
    pub fn new(shape: GradientShape, from: Point, to: Point, ink: GradientInk) -> Gradient {
        Gradient {
            shape,
            from,
            to,
            ink,
        }
    }

    #[must_use]
    pub fn shape(&self) -> GradientShape {
        self.shape
    }

    #[must_use]
    pub fn ends(&self) -> (Point, Point) {
        (self.from, self.to)
    }

    #[must_use]
    pub fn ink(&self) -> GradientInk {
        self.ink
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GradientPush {
    head: PushHeader,
    from: [f32; 2],
    to: [f32; 2],
    start: [f32; 4],
    end: [f32; 4],
    dither: f32,
    ink: u32,
    pad: [u32; 2],
}

fn dither(depth: Depth) -> f32 {
    match depth {
        Depth::Eight => 1.0 / 255.0,
        Depth::Sixteen => 1.0 / 65535.0,
        Depth::Float => 0.0,
    }
}

impl Effect for Gradient {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let (ink, start, end, alpha) = match self.ink {
            GradientInk::Colors { from, to } => (
                INK_COLORS,
                frame.space.translated([from.r, from.g, from.b, from.a]),
                frame.space.translated([to.r, to.g, to.b, to.a]),
                frame.has_alpha || from.a < 1.0 || to.a < 1.0,
            ),
            GradientInk::Fade { .. } if !frame.has_alpha => {
                return Err(Error::Invalid(
                    "fade gradient requires an alpha channel".into(),
                ));
            }
            GradientInk::Fade { from, to } => (
                INK_FADE,
                [0.0, 0.0, 0.0, from.clamp(0.0, 1.0)],
                [0.0, 0.0, 0.0, to.clamp(0.0, 1.0)],
                true,
            ),
        };
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let push = GradientPush {
            head: PushHeader::new(&frame.plane, &out)
                .mode(self.shape.code())
                .clamped(frame.depth.clamps()),
            from: [self.from.x, self.from.y],
            to: [self.to.x, self.to.y],
            start,
            end,
            dither: dither(frame.depth),
            ink,
            pad: [0; 2],
        };
        work.pass(Shader::Gradient, push, Io::new(&frame.plane, &out), 2.0)?;
        Ok(Frame {
            has_alpha: alpha,
            ..frame.with_plane(out)
        })
    }

    fn scaled(&self, factor: f32) -> Gradient {
        Gradient {
            from: self.from * factor,
            to: self.to * factor,
            ..*self
        }
    }
}

crate::work::push_header!(GradientPush);
