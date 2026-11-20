use crate::color::Color;
use crate::document::{ColorMode, Document, Version};
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::pixels::Plane;
use crate::resources::{Resolution, ResourceData, id as resource_id};
use crate::transform::Affine;
use crate::transform::pass::Pass;
use crate::transform::records::TransformPlan;
use crate::transform::retarget::Retarget;

const SNAP: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub map: Affine,
}

impl Geometry {
    pub fn new(width: u32, height: u32, map: Affine) -> Self {
        Self { width, height, map }
    }

    pub fn resize(from: (u32, u32), width: u32, height: u32) -> Self {
        let scale = |to: u32, from: u32| f64::from(to) / f64::from(from.max(1));
        Self::new(
            width,
            height,
            Affine::scale(scale(width, from.0), scale(height, from.1)),
        )
    }

    pub fn rotate(from: (u32, u32), clockwise_degrees: f64) -> Self {
        let (from_width, from_height) = (f64::from(from.0), f64::from(from.1));
        let rotation = Affine::rotation(clockwise_degrees);
        let (sin, cos) = (rotation.xy.abs(), rotation.xx.abs());
        let extent = |value: f64| (value - SNAP).ceil().max(1.0) as u32;
        let width = extent(from_width * cos + from_height * sin);
        let height = extent(from_width * sin + from_height * cos);
        let map = Affine::translation(-from_width / 2.0, -from_height / 2.0)
            .then(rotation)
            .then(Affine::translation(
                f64::from(width) / 2.0,
                f64::from(height) / 2.0,
            ));
        Self::new(width, height, map)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResampleOptions {
    pub background: Color,
    pub scale_styles: bool,
    pub resolution: Option<Resolution>,
}

impl Default for ResampleOptions {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            scale_styles: true,
            resolution: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Clamp,
    Constant(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Resampling<'a> {
    pub source: &'a Plane,
    pub weight: Option<&'a Plane>,
    pub width: u32,
    pub height: u32,
    pub map: Affine,
    pub edge: Edge,
    pub discrete: bool,
}

pub trait Resampler {
    fn resample(&mut self, request: &Resampling<'_>) -> Result<Plane>;
}

impl Document {
    pub fn resample(
        &mut self,
        geometry: Geometry,
        options: &ResampleOptions,
        resampler: &mut impl Resampler,
    ) -> Result<()> {
        Version::Psb.check_dimensions(geometry.width, geometry.height)?;
        let inverse = geometry.map.inverse().ok_or(Error::Invalid {
            what: "resample geometry determinant",
            value: 0,
        })?;
        let mut pass = Pass {
            resampler,
            forward: geometry.map,
            inverse,
            from: self.bounds(),
            to: Rect::from_size(0, 0, geometry.width, geometry.height),
            fill: self.canvas_fill(options.background),
            paint: self.native_color(options.background),
            discrete: matches!(self.color_mode, ColorMode::Indexed | ColorMode::Bitmap),
            depth: self.depth,
        };
        TransformPlan::plan(self, &mut pass)?.commit(self);
        Retarget::new(
            geometry.map,
            (self.width, self.height),
            (geometry.width, geometry.height),
        )
        .scaling_styles(options.scale_styles)
        .apply(self);
        if let Some(resolution) = options.resolution {
            self.resources.set(
                resource_id::RESOLUTION,
                ResourceData::Resolution(resolution),
            );
        }
        self.resize_to(geometry.width, geometry.height);
        Ok(())
    }
}
