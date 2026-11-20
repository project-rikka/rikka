use crate::color::Color;
use crate::document::{Document, Version};
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::layers::{ChannelId, Layer};
use crate::pixels::{Depth, Plane};
use crate::transform::Affine;
use crate::transform::records::{
    LayerGeometry, Remap, TransformPlan, mapped_mask, rebuild_channels,
};
use crate::transform::retarget::Retarget;

struct Reframe {
    canvas: Rect,
    fill: Vec<f32>,
    paint: Vec<f32>,
    depth: Depth,
}

impl Reframe {
    fn shift(&self, bounds: Rect) -> Result<Rect> {
        if bounds.is_empty() {
            return Ok(bounds);
        }
        relative(bounds, self.canvas)
    }

    fn backdrop(&self, layer: &Layer) -> Result<LayerGeometry> {
        let area = relative(self.canvas, layer.bounds)?;
        let channels = rebuild_channels(layer, self.depth, |channel, plane| {
            Ok(match channel.id {
                ChannelId::UserMask | ChannelId::RealUserMask => plane,
                ChannelId::Color(index) => plane.reframed(
                    area,
                    self.paint.get(usize::from(index)).copied().unwrap_or(0.0),
                ),
                _ => plane.reframed(area, 0.0),
            })
        })?;
        Ok(LayerGeometry {
            bounds: Rect::from_size(0, 0, self.canvas.width(), self.canvas.height()),
            mask: None,
            channels: Some(channels),
        })
    }
}

impl Remap for Reframe {
    fn remap_composite(&mut self, index: usize, plane: Plane) -> Result<Plane> {
        Ok(plane.reframed(self.canvas, self.fill[index]))
    }

    fn remap_layer(&mut self, layer: &Layer, background: bool) -> Result<LayerGeometry> {
        if background {
            return self.backdrop(layer);
        }
        Ok(LayerGeometry {
            bounds: self.shift(layer.bounds)?,
            mask: mapped_mask(layer, |bounds| self.shift(bounds))?,
            channels: None,
        })
    }
}

fn relative(rect: Rect, origin: Rect) -> Result<Rect> {
    let shift = |value: i32, by: i32| value.checked_sub(by).ok_or(Error::Oversized);
    Ok(Rect::new(
        shift(rect.left, origin.left)?,
        shift(rect.top, origin.top)?,
        shift(rect.right, origin.left)?,
        shift(rect.bottom, origin.top)?,
    ))
}

impl Document {
    pub fn crop(&mut self, area: Rect) -> Result<()> {
        if !self.bounds().encloses(&area) {
            return Err(Error::Planes("the crop area lies outside the document"));
        }
        self.resize_canvas(area, Color::WHITE)
    }

    pub fn resize_canvas(&mut self, canvas: Rect, background: Color) -> Result<()> {
        let (width, height) = (canvas.width(), canvas.height());
        Version::Psb.check_dimensions(width, height)?;
        let mut reframe = Reframe {
            canvas,
            fill: self.canvas_fill(background),
            paint: self.native_color(background),
            depth: self.depth,
        };
        TransformPlan::plan(self, &mut reframe)?.commit(self);
        Retarget::new(
            Affine::translation(-f64::from(canvas.left), -f64::from(canvas.top)),
            (self.width, self.height),
            (width, height),
        )
        .apply(self);
        self.resize_to(width, height);
        Ok(())
    }

    pub(crate) fn resize_to(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        if self.version.check_dimensions(width, height).is_err() {
            self.version = Version::Psb;
        }
    }
}
