use rikka_image::ColorType;
use rikka_image_edit::{Fill, Recipe, Step};
use rikka_types::Color;

use super::{Document, Recorded, check_memory_budget, pixels};
use crate::area::Area;
use crate::combine::Combine;
use crate::edit::SelectionAction;
use crate::error::Result;
use crate::flood::{Flood, PixelChange, Source};
use crate::history::Action;
use crate::layer::LayerId;
use crate::modify::Modify;
use crate::region::{Builder, Region};
use crate::shape::Shape;

const READBACK_BYTES_PER_PIXEL: u64 = 20;

impl Document {
    pub(super) fn select_shape(&mut self, shape: &Shape, combine: Combine) -> Option<Recorded> {
        let drawn = shape.region(self.canvas());
        let region = self.state.selection.combine(combine, drawn);
        self.select(&region, shape.selecting())
    }

    pub(super) fn magic_wand(
        &mut self,
        flood: &Flood,
        combine: Combine,
    ) -> Result<Option<Recorded>> {
        let rgba = self.rgba(flood.sampled())?;
        let filled = flood.region(&self.pixels_of(&rgba), None)?;
        let region = self.state.selection.combine(combine, filled);
        Ok(self.select(&region, SelectionAction::MagicWand))
    }

    pub(super) fn select_layer(
        &mut self,
        layer: LayerId,
        combine: Combine,
    ) -> Result<Option<Recorded>> {
        let rgba = self.rgba(Source::Layer(layer))?;
        let opaque = alpha_region(self.width(), &rgba);
        let region = self.state.selection.combine(combine, opaque);
        Ok(self.select(&region, SelectionAction::LayerPixels))
    }

    pub(super) fn modify_selection(&mut self, modify: Modify) -> Result<Option<Recorded>> {
        if self.state.selection.is_empty() {
            return Ok(None);
        }
        let region = self.state.selection.modified(modify, self.canvas())?;
        Ok(self.select(&region, modify.selecting()))
    }

    pub(super) fn move_selection(&mut self, dx: i32, dy: i32) -> Option<Recorded> {
        if (dx, dy) == (0, 0) || self.state.selection.is_empty() {
            return None;
        }
        let region = self
            .state
            .selection
            .translated(dx.into(), dy.into(), self.canvas());
        self.select(&region, SelectionAction::Move)
    }

    pub(super) fn bucket(
        &mut self,
        layer: LayerId,
        flood: &Flood,
        color: Color,
    ) -> Result<Option<Recorded>> {
        let (width, height) = (self.width(), self.height());
        let rgba = self.rgba(flood.sampled())?;
        let selection = &self.state.selection;
        let within = (!selection.is_empty()).then(|| selection.coverages(width, height));
        let region = flood.region(&self.pixels_of(&rgba), within.as_deref())?;
        let covered = match within {
            Some(_) => region.combine(Combine::Intersect, selection.clone()),
            None => region,
        };
        if covered.is_empty() {
            return Ok(None);
        }
        let step = Step::new(Fill::new(color)).masked(covered.mask(width, height)?);
        self.develop(layer, &Recipe::new().then(step), Action::Bucket)
    }

    pub(super) fn canvas(&self) -> Area {
        Area::sized(self.width(), self.height())
    }

    fn pixels_of<'a>(&self, rgba: &'a [u8]) -> PixelChange<'a> {
        PixelChange {
            width: self.width(),
            height: self.height(),
            rgba,
        }
    }

    fn rgba(&mut self, source: Source) -> Result<Vec<u8>> {
        let (width, height) = (self.width(), self.height());
        check_memory_budget(
            width,
            height,
            pixels(width, height) * READBACK_BYTES_PER_PIXEL,
        )?;
        let picture = match source {
            Source::Image => self.composited()?,
            Source::Layer(layer) => self.layer_picture(layer)?,
        };
        let image = picture.to_image()?;
        Ok(image.convert(ColorType::Rgba8).as_bytes().to_vec())
    }
}

fn alpha_region(width: u32, rgba: &[u8]) -> Region {
    let mut builder = Builder::new(0);
    for row in rgba.chunks(width as usize * 4) {
        let mut start = 0;
        while start < width as usize {
            let alpha = row[start * 4 + 3];
            let mut end = start + 1;
            while end < width as usize && row[end * 4 + 3] == alpha {
                end += 1;
            }
            builder.push(start as u32, end as u32, alpha);
            start = end;
        }
        builder.end_row();
    }
    builder.finish()
}
