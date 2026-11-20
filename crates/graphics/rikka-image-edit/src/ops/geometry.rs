use bytemuck::{Pod, Zeroable};
use rikka_image::Anchor;
use rikka_types::Color;

use crate::error::{Error, Result};
use crate::ops::matrix::{Matrix, inverse, product};
use crate::ops::passes::{CopyPush, Domain, Kernel1d, in_premultiplied, scaled_extent};
use crate::ops::resizing::resized;
use crate::ops::{Effect, Reach};
use crate::picture::Frame;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const REMAP: u32 = 1;
const FILL: u32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Resampling {
    Nearest,
    Bilinear,
    Bicubic,
    BicubicSmoother,
    #[default]
    Lanczos,
}

impl Resampling {
    pub const ALL: [Resampling; 5] = [
        Resampling::Nearest,
        Resampling::Bilinear,
        Resampling::Bicubic,
        Resampling::BicubicSmoother,
        Resampling::Lanczos,
    ];

    pub(crate) fn kernel(self) -> Kernel1d {
        match self {
            Resampling::Nearest => Kernel1d::Nearest,
            Resampling::Bilinear => Kernel1d::Triangle,
            Resampling::Bicubic => Kernel1d::CatmullRom,
            Resampling::BicubicSmoother => Kernel1d::Mitchell,
            Resampling::Lanczos => Kernel1d::Lanczos3,
        }
    }

    fn warp_mode(self) -> u32 {
        match self {
            Resampling::Nearest => 0,
            Resampling::Bilinear => 1,
            Resampling::Bicubic | Resampling::BicubicSmoother | Resampling::Lanczos => 2,
        }
    }
}

fn remapped(
    work: &mut Work,
    frame: &Frame,
    extent: [u32; 2],
    from: [i32; 2],
    quarter: u32,
    mirror: u32,
    fill: [f32; 4],
) -> Result<Frame> {
    let out = work.plane(extent, frame.plane.format())?;
    let push = CopyPush {
        head: PushHeader::new(&frame.plane, &out).mode(REMAP),
        from,
        quarter,
        mirror,
        fill,
    };
    work.pass(Shader::Copy, push, Io::new(&frame.plane, &out), 1.0)?;
    Ok(frame.with_plane(out))
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    color: Color,
}

impl Fill {
    #[must_use]
    pub fn new(color: Color) -> Fill {
        Fill { color }
    }
}

impl Effect for Fill {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let out = work.plane_like(&frame.plane, frame.depth.working())?;
        let fill = frame
            .space
            .translated([self.color.r, self.color.g, self.color.b, self.color.a]);
        let push = CopyPush {
            head: PushHeader::new(&frame.plane, &out).mode(FILL),
            from: [0, 0],
            quarter: 0,
            mirror: 0,
            fill,
        };
        work.pass(Shader::Copy, push, Io::new(&frame.plane, &out), 1.0)?;
        Ok(Frame {
            has_alpha: frame.has_alpha || self.color.a < 1.0,
            ..frame.with_plane(out)
        })
    }

    fn reach(&self, _extent: [u32; 2]) -> Reach {
        Reach::POINTWISE
    }
}

fn validated_extent(extent: [u32; 2]) -> Result<[u32; 2]> {
    match extent[0] > 0 && extent[1] > 0 && extent[0] <= 300_000 && extent[1] <= 300_000 {
        true => Ok(extent),
        false => Err(Error::Invalid(format!(
            "invalid picture size {}×{}",
            extent[0], extent[1]
        ))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resize {
    width: u32,
    height: u32,
    resampling: Resampling,
}

impl Resize {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Resize {
        Resize {
            width,
            height,
            resampling: Resampling::default(),
        }
    }

    #[must_use]
    pub fn resampling(mut self, resampling: Resampling) -> Resize {
        self.resampling = resampling;
        self
    }
}

impl Effect for Resize {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let extent = validated_extent([self.width, self.height])?;
        resized(work, frame, extent, self.resampling.kernel())
    }

    fn scaled(&self, factor: f32) -> Resize {
        let [width, height] = scaled_extent([self.width, self.height], factor);
        Resize {
            width,
            height,
            ..*self
        }
    }

    fn reshapes(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuarterTurn {
    Clockwise,
    Half,
    CounterClockwise,
}

impl Effect for QuarterTurn {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let [width, height] = frame.extent();
        let (quarter, extent) = match self {
            QuarterTurn::Clockwise => (1, [height, width]),
            QuarterTurn::Half => (2, [width, height]),
            QuarterTurn::CounterClockwise => (3, [height, width]),
        };
        remapped(work, frame, extent, [0, 0], quarter, 0, [0.0; 4])
    }

    fn reshapes(&self) -> bool {
        *self != QuarterTurn::Half
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flip {
    Horizontal,
    Vertical,
}

impl Effect for Flip {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let mirror = match self {
            Flip::Horizontal => 1,
            Flip::Vertical => 2,
        };
        remapped(work, frame, frame.extent(), [0, 0], 0, mirror, [0.0; 4])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Crop {
    #[must_use]
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Crop {
        Crop {
            x: x as f32,
            y: y as f32,
            width: width as f32,
            height: height as f32,
        }
    }
}

impl Effect for Crop {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let extent = validated_extent([self.width.round() as u32, self.height.round() as u32])?;
        let from = [self.x.round() as i32, self.y.round() as i32];
        let [width, height] = frame.extent();
        let inside = from[0] >= 0
            && from[1] >= 0
            && from[0] as u32 + extent[0] <= width
            && from[1] as u32 + extent[1] <= height;
        let cropped = remapped(work, frame, extent, from, 0, 0, [0.0; 4])?;
        Ok(Frame {
            has_alpha: frame.has_alpha || !inside,
            ..cropped
        })
    }

    fn scaled(&self, factor: f32) -> Crop {
        Crop {
            x: self.x * factor,
            y: self.y * factor,
            width: (self.width * factor).max(1.0),
            height: (self.height * factor).max(1.0),
        }
    }

    fn reshapes(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasSize {
    width: f32,
    height: f32,
    anchor: Anchor,
    fill: Color,
}

impl CanvasSize {
    #[must_use]
    pub fn new(width: u32, height: u32) -> CanvasSize {
        CanvasSize {
            width: width as f32,
            height: height as f32,
            anchor: Anchor::Center,
            fill: Color::TRANSPARENT,
        }
    }

    #[must_use]
    pub fn anchor(mut self, anchor: Anchor) -> CanvasSize {
        self.anchor = anchor;
        self
    }

    #[must_use]
    pub fn fill(mut self, fill: Color) -> CanvasSize {
        self.fill = fill;
        self
    }
}

impl Effect for CanvasSize {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let extent = validated_extent([self.width.round() as u32, self.height.round() as u32])?;
        let [width, height] = frame.extent();
        let (x, y) = self.anchor.offset((extent[0], extent[1]), (width, height));
        let fill = frame
            .space
            .translated([self.fill.r, self.fill.g, self.fill.b, self.fill.a]);
        let grown = extent[0] > width || extent[1] > height;
        let resized = remapped(work, frame, extent, [-x as i32, -y as i32], 0, 0, fill)?;
        Ok(Frame {
            has_alpha: frame.has_alpha || (grown && self.fill.a < 1.0),
            ..resized
        })
    }

    fn scaled(&self, factor: f32) -> CanvasSize {
        CanvasSize {
            width: (self.width * factor).max(1.0),
            height: (self.height * factor).max(1.0),
            ..*self
        }
    }

    fn reshapes(&self) -> bool {
        true
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WarpPush {
    head: PushHeader,
    rows: [[f32; 4]; 3],
    fill: [f32; 4],
}

fn warped(
    work: &mut Work,
    frame: &Frame,
    extent: [u32; 2],
    back: Matrix,
    resampling: Resampling,
    fill: Color,
) -> Result<Frame> {
    let extent = validated_extent(extent)?;
    let fill = frame
        .space
        .linear_premultiplied([fill.r, fill.g, fill.b, fill.a]);
    let row = |row: [f64; 3]| [row[0] as f32, row[1] as f32, row[2] as f32, 0.0];
    let warped = in_premultiplied(work, frame, Domain::Linear, |work, linear| {
        let out = work.plane(extent, linear.format())?;
        let push = WarpPush {
            head: PushHeader::new(&linear, &out).mode(resampling.warp_mode()),
            rows: back.map(row),
            fill,
        };
        work.pass(Shader::Warp, push, Io::new(&linear, &out), 64.0)?;
        Ok(out)
    })?;
    Ok(Frame {
        has_alpha: frame.has_alpha || fill[3] < 1.0,
        ..warped
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotate {
    degrees: f32,
    expand: bool,
    resampling: Resampling,
    fill: Color,
}

impl Rotate {
    #[must_use]
    pub fn new(degrees: f32) -> Rotate {
        Rotate {
            degrees,
            expand: true,
            resampling: Resampling::Bicubic,
            fill: Color::TRANSPARENT,
        }
    }

    #[must_use]
    pub fn expand(mut self, expand: bool) -> Rotate {
        self.expand = expand;
        self
    }

    #[must_use]
    pub fn resampling(mut self, resampling: Resampling) -> Rotate {
        self.resampling = resampling;
        self
    }

    #[must_use]
    pub fn fill(mut self, fill: Color) -> Rotate {
        self.fill = fill;
        self
    }
}

impl Effect for Rotate {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let [width, height] = frame.extent().map(f64::from);
        let (sin, cos) = f64::from(self.degrees).to_radians().sin_cos();
        let extent = match self.expand {
            true => [
                (width * cos.abs() + height * sin.abs()).round().max(1.0) as u32,
                (width * sin.abs() + height * cos.abs()).round().max(1.0) as u32,
            ],
            false => frame.extent(),
        };
        let (cx, cy) = (f64::from(extent[0]) / 2.0, f64::from(extent[1]) / 2.0);
        let back = [
            [cos, sin, width / 2.0 - cos * cx - sin * cy],
            [-sin, cos, height / 2.0 + sin * cx - cos * cy],
            [0.0, 0.0, 1.0],
        ];
        warped(work, frame, extent, back, self.resampling, self.fill)
    }

    fn reshapes(&self) -> bool {
        self.expand
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    forward: Matrix,
    width: f32,
    height: f32,
    resampling: Resampling,
    fill: Color,
}

impl Transform {
    #[must_use]
    pub fn new(forward: [[f64; 3]; 3], width: u32, height: u32) -> Transform {
        Transform {
            forward,
            width: width as f32,
            height: height as f32,
            resampling: Resampling::Bicubic,
            fill: Color::TRANSPARENT,
        }
    }

    pub fn corners(
        source: (u32, u32),
        corners: [(f64, f64); 4],
        width: u32,
        height: u32,
    ) -> Result<Transform> {
        let (w, h) = (f64::from(source.0), f64::from(source.1));
        let from = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
        let forward = homography(from, corners)
            .ok_or_else(|| Error::Invalid("corners do not form a quadrilateral".into()))?;
        Ok(Transform::new(forward, width, height))
    }

    #[must_use]
    pub fn resampling(mut self, resampling: Resampling) -> Transform {
        self.resampling = resampling;
        self
    }

    #[must_use]
    pub fn fill(mut self, fill: Color) -> Transform {
        self.fill = fill;
        self
    }
}

impl Effect for Transform {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let back = inverse(&self.forward);
        if back.iter().flatten().any(|value| !value.is_finite()) {
            return Err(Error::Invalid("transform matrix is singular".into()));
        }
        let extent = [self.width.round() as u32, self.height.round() as u32];
        warped(work, frame, extent, back, self.resampling, self.fill)
    }

    fn scaled(&self, factor: f32) -> Transform {
        let f = f64::from(factor);
        let scale = [[f, 0.0, 0.0], [0.0, f, 0.0], [0.0, 0.0, 1.0]];
        let unscale = [[1.0 / f, 0.0, 0.0], [0.0, 1.0 / f, 0.0], [0.0, 0.0, 1.0]];
        Transform {
            forward: product(&product(&scale, &self.forward), &unscale),
            width: (self.width * factor).max(1.0),
            height: (self.height * factor).max(1.0),
            ..*self
        }
    }

    fn reshapes(&self) -> bool {
        true
    }
}

fn homography(from: [(f64, f64); 4], to: [(f64, f64); 4]) -> Option<Matrix> {
    let mut system = [[0.0f64; 9]; 8];
    for (index, (&(x, y), &(u, v))) in from.iter().zip(to.iter()).enumerate() {
        system[index * 2] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        system[index * 2 + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for column in 0..8 {
        let pivot = (column..8)
            .max_by(|&a, &b| system[a][column].abs().total_cmp(&system[b][column].abs()))?;
        if system[pivot][column].abs() < 1e-12 {
            return None;
        }
        system.swap(column, pivot);
        let lead = system[column][column];
        for value in &mut system[column] {
            *value /= lead;
        }
        for row in 0..8 {
            if row != column {
                let factor = system[row][column];
                let source = system[column];
                for (value, from) in system[row].iter_mut().zip(source) {
                    *value -= factor * from;
                }
            }
        }
    }
    let h: Vec<f64> = system.iter().map(|row| row[8]).collect();
    Some([[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]])
}

crate::work::push_header!(WarpPush);
