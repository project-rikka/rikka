use bytemuck::{Pod, Zeroable};

use crate::color::Intent;
use crate::error::{Error, Result};
use crate::ops::Effect;
use crate::ops::convert::converted;
use crate::ops::geometry::Resampling;
use crate::ops::matrix::{Matrix, inverse};
use crate::ops::passes::{Domain, Kernel1d, premultiplied, resampled};
use crate::picture::{Frame, Picture};
use crate::plane::PlaneRef;
use crate::recipe::Blend;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const NEAREST: f32 = 0.0;
const BILINEAR: f32 = 1.0;
const BICUBIC: f32 = 2.0;
const REPLACE: u32 = 64;

const PREFILTER_BELOW: f64 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    picture: Picture,
    matrix: [[f32; 3]; 3],
    opacity: f32,
    blend: Blend,
    radius: f32,
    resampling: Resampling,
    replacing: bool,
    mirrored: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PlacePush {
    head: PushHeader,
    base: PlaneRef,
    params: u64,
    tables: u64,
    transfer: u32,
    pad: u32,
}

impl Place {
    #[must_use]
    pub fn new(picture: Picture, matrix: [[f32; 3]; 3]) -> Place {
        Place {
            picture,
            matrix,
            opacity: 1.0,
            blend: Blend::Normal,
            radius: 0.0,
            resampling: Resampling::Bilinear,
            replacing: false,
            mirrored: false,
        }
    }

    #[must_use]
    pub fn replacing(mut self) -> Place {
        self.replacing = true;
        self
    }

    #[must_use]
    pub fn mirrored(mut self) -> Place {
        self.mirrored = true;
        self
    }

    #[must_use]
    pub fn opacity(mut self, opacity: f32) -> Place {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    #[must_use]
    pub fn blend(mut self, blend: Blend) -> Place {
        self.blend = blend;
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Place {
        self.radius = radius.max(0.0);
        self
    }

    #[must_use]
    pub fn resampling(mut self, resampling: Resampling) -> Place {
        self.resampling = resampling;
        self
    }

    #[must_use]
    pub fn picture(&self) -> &Picture {
        &self.picture
    }

    #[must_use]
    pub fn matrix(&self) -> [[f32; 3]; 3] {
        self.matrix
    }

    fn forward(&self) -> Matrix {
        self.matrix.map(|row| row.map(f64::from))
    }

    fn filter(&self) -> f32 {
        match self.resampling {
            Resampling::Nearest => NEAREST,
            Resampling::Bilinear => BILINEAR,
            Resampling::Bicubic | Resampling::BicubicSmoother | Resampling::Lanczos => BICUBIC,
        }
    }
}

impl Effect for Place {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let forward = self.forward();
        let back = inverse(&forward);
        if back.iter().flatten().any(|value| !value.is_finite()) {
            return Err(Error::Invalid("placement matrix is singular".into()));
        }
        if self.opacity <= 0.0 && !self.replacing {
            return Ok(frame.clone());
        }
        let placed = converted(
            work,
            self.picture.frame(),
            frame.space.clone(),
            Intent::RelativeColorimetric,
        )?;
        let linear = premultiplied(work, &placed, Domain::Linear)?;
        let [width, height] = placed.extent();
        let reduced = match self.resampling {
            Resampling::Nearest => [width, height],
            _ => prefiltered([width, height], magnification(&forward, [width, height])),
        };
        let source = match reduced == [width, height] {
            true => linear,
            false => resampled(work, &linear, reduced, Kernel1d::Triangle)?,
        };
        let row = |row: [f64; 3]| row.map(|value| value as f32);
        let mut params: Vec<f32> = back.into_iter().flat_map(row).collect();
        params.extend([
            width as f32,
            height as f32,
            reduced[0] as f32 / width as f32,
            reduced[1] as f32 / height as f32,
            self.radius.min(width.min(height) as f32 / 2.0),
            self.opacity,
            self.filter(),
            f32::from(u8::from(self.mirrored)),
        ]);
        let params = work.constants(&params)?;
        let tables = frame.space.tables(work)?;
        let out = work.plane(frame.extent(), frame.depth.working())?;
        let mode = match self.replacing {
            true => REPLACE,
            false => self.blend.code(),
        };
        let push = PlacePush {
            head: PushHeader::new(&source, &out)
                .mode(mode)
                .clamped(frame.depth.clamps()),
            base: frame.plane.reference(),
            params: params.address(),
            tables: tables.address(),
            transfer: frame.space.transfer(),
            pad: 0,
        };
        let io = Io::new(&source, &out)
            .read(&frame.plane)
            .read_buffer(&params)
            .read_buffer(&tables);
        work.pass(Shader::Place, push, io, 24.0)?;
        Ok(Frame {
            has_alpha: frame.has_alpha || self.replacing,
            ..frame.with_plane(out)
        })
    }

    fn scaled(&self, factor: f32) -> Place {
        let [top, middle, bottom] = self.matrix;
        let matrix = [
            top.map(|value| value * factor),
            middle.map(|value| value * factor),
            bottom,
        ];
        Place {
            matrix,
            ..self.clone()
        }
    }
}

fn magnification(forward: &Matrix, extent: [u32; 2]) -> [f64; 2] {
    let [width, height] = extent.map(f64::from);
    let points = [
        (0.0, 0.0),
        (width, 0.0),
        (0.0, height),
        (width, height),
        (width / 2.0, height / 2.0),
    ];
    let [[a, b, c], [d, e, f], [g, h, i]] = *forward;
    points
        .into_iter()
        .filter_map(|(u, v)| {
            let w = g * u + h * v + i;
            if w <= 0.0 {
                return None;
            }
            let x = (a * u + b * v + c) / w;
            let y = (d * u + e * v + f) / w;
            let along_u = ((a - x * g) / w).hypot((d - y * g) / w);
            let along_v = ((b - x * h) / w).hypot((e - y * h) / w);
            Some([along_u, along_v])
        })
        .fold([0.0f64, 0.0f64], |most, now| {
            [most[0].max(now[0]), most[1].max(now[1])]
        })
}

fn prefiltered(extent: [u32; 2], magnification: [f64; 2]) -> [u32; 2] {
    let mut reduced = extent;
    for axis in 0..2 {
        let scale = magnification[axis];
        if scale > 0.0 && scale < PREFILTER_BELOW {
            let side = (f64::from(extent[axis]) * scale * 2.0).ceil().max(1.0) as u32;
            reduced[axis] = side.min(extent[axis]);
        }
    }
    reduced
}

crate::work::push_header!(PlacePush);
