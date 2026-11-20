use paint_document::{Area, Shape};
use rikka_image_edit::{FillRule, Mask, Path};

use crate::svg::outlines;

#[derive(Clone, Debug, PartialEq)]
pub enum MaskShape {
    Rectangle([f32; 4]),
    Ellipse([f32; 4]),
    Path(Path, FillRule),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaskOrder {
    pub shape: MaskShape,
    pub feather: f32,
    pub invert: bool,
}

impl MaskOrder {
    pub fn mask(&self, width: u32, height: u32) -> Result<Mask, String> {
        let shaped = match &self.shape {
            MaskShape::Rectangle([x, y, w, h]) => Mask::rectangle(*x, *y, *w, *h),
            MaskShape::Ellipse([x, y, w, h]) => Mask::ellipse(*x, *y, *w, *h),
            MaskShape::Path(path, rule) => Shape::polygon(outlines(path), *rule)
                .region(Area::sized(width, height))
                .mask(width, height)
                .map_err(|error| format!("the mask could not be made: {error}"))?,
        };
        let feathered = shaped.feathered(self.feather);
        Ok(match self.invert {
            true => feathered.inverted(),
            false => feathered,
        })
    }
}
