use crate::document::Document;
use crate::error::Result;
use crate::geometry::Rect;
use crate::layers::Layer;
use crate::pixels::{Depth, Plane};
use crate::transform::orientation::{Axis, Orientation, QuarterTurn};
use crate::transform::records::{
    LayerGeometry, Remap, TransformPlan, mapped_mask, rebuild_channels,
};
use crate::transform::retarget::Retarget;

struct Orient {
    orientation: Orientation,
    width: u32,
    height: u32,
    depth: Depth,
}

impl Orient {
    fn rect(&self, rect: Rect) -> Result<Rect> {
        if rect.is_empty() {
            return Ok(rect);
        }
        self.orientation.rect(rect, self.width, self.height)
    }
}

impl Remap for Orient {
    fn remap_composite(&mut self, _: usize, plane: Plane) -> Result<Plane> {
        self.orientation.plane(&plane)
    }

    fn remap_layer(&mut self, layer: &Layer, _: bool) -> Result<LayerGeometry> {
        Ok(LayerGeometry {
            bounds: self.rect(layer.bounds)?,
            mask: mapped_mask(layer, |bounds| self.rect(bounds))?,
            channels: Some(rebuild_channels(layer, self.depth, |_, plane| {
                self.orientation.plane(&plane)
            })?),
        })
    }
}

impl Document {
    pub fn flip(&mut self, axis: Axis) -> Result<()> {
        self.orient(Orientation::from(axis))
    }

    pub fn rotate_quarter(&mut self, turn: QuarterTurn) -> Result<()> {
        self.orient(Orientation::from(turn))
    }

    fn orient(&mut self, orientation: Orientation) -> Result<()> {
        let (width, height) = (self.width, self.height);
        let mut orient = Orient {
            orientation,
            width,
            height,
            depth: self.depth,
        };
        TransformPlan::plan(self, &mut orient)?.commit(self);
        let size = orientation.size(width, height);
        Retarget::new(orientation.canvas(width, height), (width, height), size).apply(self);
        self.resize_to(size.0, size.1);
        Ok(())
    }
}
