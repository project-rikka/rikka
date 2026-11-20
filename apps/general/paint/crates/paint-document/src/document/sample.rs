use rikka_image::Image;
use rikka_image_edit::{ConvertProfile, Crop, Picture, Profile, Recipe};
use rikka_types::Color;

use super::{Document, no_layer};
use crate::error::{Error, Result};
use crate::layer::LayerId;
use crate::state::Content;

impl Document {
    pub fn sample(&mut self, x: u32, y: u32, size: u32, layer: Option<LayerId>) -> Result<Color> {
        let (width, height) = (self.width(), self.height());
        if x >= width || y >= height {
            return Err(Error::OutsideImage {
                x: x as f32,
                y: y as f32,
                width,
                height,
            });
        }
        let source = match layer {
            Some(id) => self.layer_picture(id)?,
            None => self.composited()?,
        };
        let reach = size.max(1) / 2;
        let (left, top) = (x.saturating_sub(reach), y.saturating_sub(reach));
        let (right, bottom) = ((x + reach + 1).min(width), (y + reach + 1).min(height));
        let mut recipe = Recipe::new().then(Crop::new(left, top, right - left, bottom - top));
        if let Some(profile) = source.profile()
            && *profile != Profile::srgb()
        {
            recipe.push(ConvertProfile::new(Profile::srgb()));
        }
        let window = source
            .develop(&recipe)
            .and_then(|window| window.to_image())?;
        Ok(average(&window))
    }

    pub(super) fn composited(&mut self) -> Result<Picture> {
        if let Some((revision, picture)) = &self.sampled
            && *revision == self.revision
        {
            return Ok(picture.clone());
        }
        let picture = self.composite()?;
        self.sampled = Some((self.revision, picture.clone()));
        Ok(picture)
    }

    pub fn layer_picture(&self, id: LayerId) -> Result<Picture> {
        match &self.state.content {
            Content::Picture { picture, .. } => match id == LayerId(0) {
                true => Ok(picture.clone()),
                false => Err(no_layer(id)),
            },
            Content::Photoshop { .. } => Ok(self.placed(id)?.picture),
        }
    }
}

fn average(window: &Image) -> Color {
    let (width, height) = window.dimensions();
    let mut sum = [0.0f32; 4];
    for y in 0..height {
        for x in 0..width {
            if let Some(color) = window.pixel(x, y) {
                sum[0] += color.r * color.a;
                sum[1] += color.g * color.a;
                sum[2] += color.b * color.a;
                sum[3] += color.a;
            }
        }
    }
    if sum[3] <= 0.0 {
        return Color::TRANSPARENT;
    }
    let count = (width * height).max(1) as f32;
    Color::rgba(
        sum[0] / sum[3],
        sum[1] / sum[3],
        sum[2] / sum[3],
        sum[3] / count,
    )
}
