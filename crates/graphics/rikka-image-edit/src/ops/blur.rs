use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::ops::passes::{
    Domain, Kernel1d, arith, gaussian, gaussian_reach, in_premultiplied, resampled,
};
use crate::ops::{Effect, Reach};
use crate::picture::{Depth, Frame};
use crate::plane::Plane;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const GATHER_REACH: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaussianBlur {
    radius: f32,
}

impl GaussianBlur {
    #[must_use]
    pub fn new(radius: f32) -> GaussianBlur {
        GaussianBlur {
            radius: radius.clamp(0.0, 1000.0),
        }
    }
}

impl Effect for GaussianBlur {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            gaussian(work, &linear, self.radius)
        })
    }

    fn scaled(&self, factor: f32) -> GaussianBlur {
        GaussianBlur::new(self.radius * factor)
    }

    fn reach(&self, _extent: [u32; 2]) -> Reach {
        gaussian_reach(self.radius).map_or(Reach::Global, |halo| Reach::Everywhere { halo })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BoxPush {
    head: PushHeader,
    radius: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxBlur {
    radius: f32,
}

impl BoxBlur {
    #[must_use]
    pub fn new(radius: f32) -> BoxBlur {
        BoxBlur {
            radius: radius.clamp(0.0, 2000.0),
        }
    }
}

pub(crate) fn box_blur(work: &mut Work, plane: &Plane, radius: i32) -> Result<Plane> {
    if radius < 1 {
        return Ok(plane.clone());
    }
    let mut current = plane.clone();
    for mode in [0u32, 1] {
        let out = work.plane(plane.extent(), plane.format())?;
        let lines = match mode {
            0 => plane.height(),
            _ => plane.width(),
        };
        let push = BoxPush {
            head: PushHeader::new(&current, &out).mode(mode),
            radius,
        };
        let cost = f64::from(plane.width()) * f64::from(plane.height()) * 2.0;
        work.groups(
            Shader::Box,
            push,
            Io::new(&current, &out),
            [lines.div_ceil(64), 1, 1],
            cost,
        )?;
        current = out;
    }
    Ok(current)
}

impl Effect for BoxBlur {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            box_blur(work, &linear, self.radius.round() as i32)
        })
    }

    fn scaled(&self, factor: f32) -> BoxBlur {
        BoxBlur::new(self.radius * factor)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MotionPush {
    head: PushHeader,
    center: [f32; 2],
    amount: f32,
    angle: f32,
    density: f32,
    pad: f32,
}

const LINEAR: u32 = 0;
const SPIN: u32 = 1;
const ZOOM: u32 = 2;

fn motion(
    work: &mut Work,
    plane: &Plane,
    push: impl Fn(&Plane, &Plane) -> MotionPush,
    taps: f64,
) -> Result<Plane> {
    let out = work.plane(plane.extent(), plane.format())?;
    work.pass(
        Shader::Motion,
        push(plane, &out),
        Io::new(plane, &out),
        taps.max(2.0) * 4.0,
    )?;
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionBlur {
    angle: f32,
    distance: f32,
}

impl MotionBlur {
    #[must_use]
    pub fn new(angle: f32, distance: f32) -> MotionBlur {
        MotionBlur {
            angle: angle.clamp(-360.0, 360.0),
            distance: distance.clamp(0.0, 2000.0),
        }
    }

    pub(crate) fn blur_premultiplied(&self, work: &mut Work, plane: &Plane) -> Result<Plane> {
        motion(
            work,
            plane,
            |src, dst| MotionPush {
                head: PushHeader::new(src, dst).mode(LINEAR),
                center: [0.0, 0.0],
                amount: self.distance,
                angle: self.angle.to_radians(),
                density: 1.0,
                pad: 0.0,
            },
            f64::from(self.distance),
        )
    }
}

impl Effect for MotionBlur {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            self.blur_premultiplied(work, &linear)
        })
    }
    fn scaled(&self, factor: f32) -> MotionBlur {
        MotionBlur::new(self.angle, self.distance * factor)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Quality {
    Draft,
    #[default]
    Good,
    Best,
}

impl Quality {
    fn density(self) -> f32 {
        match self {
            Quality::Draft => 0.25,
            Quality::Good => 0.6,
            Quality::Best => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialBlur {
    spin: bool,
    amount: f32,
    center: (f32, f32),
    quality: Quality,
}

impl RadialBlur {
    #[must_use]
    pub fn spin(degrees: f32) -> RadialBlur {
        RadialBlur {
            spin: true,
            amount: degrees.clamp(0.0, 360.0),
            center: (0.5, 0.5),
            quality: Quality::default(),
        }
    }

    #[must_use]
    pub fn zoom(amount: f32) -> RadialBlur {
        RadialBlur {
            spin: false,
            amount: amount.clamp(0.0, 100.0),
            center: (0.5, 0.5),
            quality: Quality::default(),
        }
    }

    #[must_use]
    pub fn center(mut self, x: f32, y: f32) -> RadialBlur {
        self.center = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        self
    }

    #[must_use]
    pub fn quality(mut self, quality: Quality) -> RadialBlur {
        self.quality = quality;
        self
    }
}

impl Effect for RadialBlur {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let [width, height] = frame.extent();
        let center = [self.center.0 * width as f32, self.center.1 * height as f32];
        let (mode, amount) = match self.spin {
            true => (SPIN, self.amount.to_radians()),
            false => (ZOOM, self.amount / 200.0),
        };
        let reach =
            f64::from(width.max(height)) * f64::from(amount) * f64::from(self.quality.density());
        in_premultiplied(work, frame, Domain::Linear, |work, linear| {
            motion(
                work,
                &linear,
                |src, dst| MotionPush {
                    head: PushHeader::new(src, dst).mode(mode),
                    center,
                    amount,
                    angle: 0.0,
                    density: self.quality.density(),
                    pad: 0.0,
                },
                reach,
            )
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LensPush {
    head: PushHeader,
    radius: f32,
    blades: u32,
    rotation: f32,
    curvature: f32,
    brightness: f32,
    threshold: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LensBlur {
    radius: f32,
    blades: u32,
    rotation: f32,
    curvature: f32,
    brightness: f32,
    threshold: f32,
}

impl LensBlur {
    #[must_use]
    pub fn new(radius: f32) -> LensBlur {
        LensBlur {
            radius: radius.clamp(0.0, 500.0),
            blades: 0,
            rotation: 0.0,
            curvature: 0.0,
            brightness: 0.0,
            threshold: 1.0,
        }
    }

    #[must_use]
    pub fn iris(mut self, blades: u32, rotation: f32, curvature: f32) -> LensBlur {
        self.blades = match blades {
            0..=2 => 0,
            blades => blades.min(8),
        };
        self.rotation = rotation;
        self.curvature = curvature.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn specular(mut self, brightness: f32, threshold: f32) -> LensBlur {
        self.brightness = brightness.clamp(0.0, 100.0);
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    fn push(&self, src: &Plane, dst: &Plane, mode: u32, radius: f32) -> LensPush {
        LensPush {
            head: PushHeader::new(src, dst).mode(mode),
            radius,
            blades: self.blades,
            rotation: self.rotation.to_radians(),
            curvature: self.curvature,
            brightness: self.brightness / 10.0,
            threshold: self.threshold,
        }
    }
}

impl LensBlur {
    pub(crate) fn blur_premultiplied(&self, work: &mut Work, linear: &Plane) -> Result<Plane> {
        if self.radius < 0.5 {
            return Ok(linear.clone());
        }
        let levels = (self.radius / GATHER_REACH).log2().ceil().max(0.0) as i32;
        let factor = 2f32.powi(levels);
        let small = [
            (linear.width() as f32 / factor).ceil().max(1.0) as u32,
            (linear.height() as f32 / factor).ceil().max(1.0) as u32,
        ];
        let reduced = match levels {
            0 => linear.clone(),
            _ => resampled(work, linear, small, Kernel1d::Box)?,
        };
        let radius = self.radius / factor;
        let blurred = work.plane(reduced.extent(), reduced.format())?;
        let taps = f64::from((2.0 * radius.ceil() + 1.0).powi(2));
        let push = self.push(&reduced, &blurred, 0, radius);
        work.pass(Shader::Lens, push, Io::new(&reduced, &blurred), taps)?;
        match levels {
            0 => Ok(blurred),
            _ => resampled(work, &blurred, linear.extent(), Kernel1d::CatmullRom),
        }
    }
}

impl Effect for LensBlur {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        in_premultiplied(work, frame, Domain::Linear, |work, mut linear| {
            if self.brightness > 0.0 {
                let highlighted = work.plane(linear.extent(), linear.format())?;
                let push = self.push(&linear, &highlighted, 1, self.radius);
                work.pass(Shader::Lens, push, Io::new(&linear, &highlighted), 2.0)?;
                linear = highlighted;
            }
            self.blur_premultiplied(work, &linear)
        })
    }

    fn scaled(&self, factor: f32) -> LensBlur {
        LensBlur {
            radius: self.radius * factor,
            ..*self
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SurfacePush {
    head: PushHeader,
    radius: i32,
    threshold: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceBlur {
    radius: f32,
    threshold: f32,
}

impl SurfaceBlur {
    #[must_use]
    pub fn new(radius: f32, threshold: f32) -> SurfaceBlur {
        SurfaceBlur {
            radius: radius.clamp(1.0, 100.0),
            threshold: threshold.clamp(2.0, 255.0),
        }
    }
}

impl Effect for SurfaceBlur {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let radius = self.radius.round().max(1.0) as i32;
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let push = SurfacePush {
            head: PushHeader::new(&frame.plane, &out).clamped(frame.depth.clamps()),
            radius,
            threshold: self.threshold / 255.0,
        };
        let taps = f64::from((2 * radius + 1).pow(2));
        work.pass(Shader::Surface, push, Io::new(&frame.plane, &out), taps)?;
        Ok(frame.with_plane(out))
    }

    fn scaled(&self, factor: f32) -> SurfaceBlur {
        SurfaceBlur {
            radius: (self.radius * factor).max(1.0),
            ..*self
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MedianPush {
    head: PushHeader,
    radius: i32,
    steps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Median {
    radius: f32,
}

impl Median {
    #[must_use]
    pub fn new(radius: f32) -> Median {
        Median {
            radius: radius.clamp(1.0, 500.0),
        }
    }
}

impl Effect for Median {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let radius = self.radius.round().max(1.0) as i32;
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let steps = match frame.depth {
            Depth::Eight => 12,
            Depth::Sixteen | Depth::Float => 18,
        };
        let push = MedianPush {
            head: PushHeader::new(&frame.plane, &out).clamped(frame.depth.clamps()),
            radius,
            steps,
        };
        let taps = f64::from((2 * radius + 1).pow(2)) * f64::from(steps + 1);
        work.pass(Shader::Median, push, Io::new(&frame.plane, &out), taps)?;
        Ok(frame.with_plane(out))
    }

    fn scaled(&self, factor: f32) -> Median {
        Median::new((self.radius * factor).max(1.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiltShift {
    center: f32,
    angle: f32,
    focus: f32,
    transition: f32,
    radius: f32,
}

impl TiltShift {
    #[must_use]
    pub fn new(radius: f32) -> TiltShift {
        TiltShift {
            center: 0.5,
            angle: 0.0,
            focus: 0.1,
            transition: 0.2,
            radius: radius.clamp(0.0, 500.0),
        }
    }

    #[must_use]
    pub fn line(mut self, center: f32, angle: f32) -> TiltShift {
        self.center = center.clamp(0.0, 1.0);
        self.angle = angle;
        self
    }

    #[must_use]
    pub fn band(mut self, focus: f32, transition: f32) -> TiltShift {
        self.focus = focus.clamp(0.0, 1.0);
        self.transition = transition.clamp(0.001, 1.0);
        self
    }
}

impl Effect for TiltShift {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let k = [self.center, sin, cos, self.focus];
        in_premultiplied(work, frame, Domain::Linear, |work, sharp| {
            let mut current = sharp.clone();
            for segment in 0..3 {
                let blurred = gaussian(work, &sharp, self.radius * (segment as f32 + 1.0) / 3.0)?;
                current = arith(
                    work,
                    &current,
                    &blurred,
                    arith::FOCUS,
                    k,
                    [self.transition, segment as f32, 0.0, 0.0],
                )?;
            }
            Ok(current)
        })
    }

    fn scaled(&self, factor: f32) -> TiltShift {
        TiltShift {
            radius: self.radius * factor,
            ..*self
        }
    }
}

crate::work::push_header!(MotionPush, LensPush, SurfacePush, MedianPush);
