use rikka_capture::{Region, Shot, ShotOptions, Source};
use rikka_image::{ColorType, Filter, Image};
use rikka_types::{Color, Rect, Size};
use shell_kit::Placement;

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub(crate) struct Aimed {
    pub(crate) output: String,
    pub(crate) region: Region,
    pub(crate) whole: bool,
}

#[derive(Debug)]
pub struct Panel {
    pub name: String,
    pub(crate) bounds: Rect,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Panel {
    #[must_use]
    pub(crate) fn scale(&self) -> f32 {
        match self.bounds.width() >= 1.0 {
            true => self.width as f32 / self.bounds.width(),
            false => 1.0,
        }
    }

    fn pixels_of(&self, part: Rect) -> Option<Region> {
        let scale = self.scale();
        let at = |along: f32, most: u32| ((along * scale).round().max(0.0) as u32).min(most);
        let span = |across: f32, left: u32| ((across * scale).round().max(1.0) as u32).min(left);
        let x = at(part.min_x() - self.bounds.min_x(), self.width);
        let y = at(part.min_y() - self.bounds.min_y(), self.height);
        let width = span(part.width(), self.width - x);
        let height = span(part.height(), self.height - y);
        (width > 0 && height > 0).then(|| Region::new(x as i32, y as i32, width, height))
    }

    fn cut(&self, part: Rect) -> Result<Image> {
        let cut = self.pixels_of(part).ok_or(Error::NothingSelected)?;
        let (x, y, width, height) = (cut.x as u32, cut.y as u32, cut.width, cut.height);

        let stride = self.width as usize * 4;
        let row = width as usize * 4;
        let mut pixels = Vec::with_capacity(row * height as usize);
        for line in 0..height as usize {
            let start = (y as usize + line) * stride + x as usize * 4;
            pixels.extend_from_slice(&self.pixels[start..start + row]);
        }
        Ok(Image::from_rgba8(width, height, pixels)?)
    }
}

pub struct Frozen {
    panels: Vec<Panel>,
}

impl std::fmt::Debug for Frozen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frozen")
            .field("panels", &self.panels.len())
            .finish_non_exhaustive()
    }
}

impl Frozen {
    pub fn take(places: &[Placement], cursor: bool) -> Result<Frozen> {
        if places.is_empty() {
            return Err(Error::NoScreen);
        }
        let panels = std::thread::scope(|scope| {
            let captures: Vec<_> = places
                .iter()
                .map(|place| scope.spawn(move || capture(place, cursor)))
                .collect();
            captures
                .into_iter()
                .map(|capture| capture.join().unwrap_or(Err(Error::NoScreen)))
                .collect::<Result<Vec<Panel>>>()
        })?;
        Ok(Frozen { panels })
    }

    #[must_use]
    pub fn into_panels(self) -> Vec<Panel> {
        self.panels
    }

    #[must_use]
    pub(crate) fn panels(&self) -> &[Panel] {
        &self.panels
    }

    #[must_use]
    pub(crate) fn bounds(&self) -> Rect {
        self.panels
            .iter()
            .map(|panel| panel.bounds)
            .reduce(|union, bounds| union.union(&bounds))
            .unwrap_or(Rect::ZERO)
    }

    #[must_use]
    pub(crate) fn panel(&self, name: &str) -> Option<&Panel> {
        self.panels
            .iter()
            .find(|panel| panel.name == name)
            .or_else(|| self.panels.first().filter(|_| self.panels.len() == 1))
    }

    #[must_use]
    pub(crate) fn aim(&self, region: Rect) -> Option<Aimed> {
        let panel = self
            .panels
            .iter()
            .max_by(|one, other| covered(one, region).total_cmp(&covered(other, region)))?;
        let region = panel.pixels_of(region.intersection(&panel.bounds)?)?;
        Some(Aimed {
            output: panel.name.clone(),
            whole: region == Region::new(0, 0, panel.width, panel.height),
            region,
        })
    }

    fn covered_by(&self, region: Rect) -> Option<&Panel> {
        self.panels
            .iter()
            .find(|panel| panel.bounds.contains_rect(&region))
    }

    #[must_use]
    pub(crate) fn scale(&self, region: Rect) -> f32 {
        self.panels
            .iter()
            .filter(|panel| panel.bounds.intersects(&region))
            .map(Panel::scale)
            .fold(1.0, f32::max)
    }

    pub(super) fn crop(&self, region: Rect) -> Result<Image> {
        let region = region
            .intersection(&self.bounds())
            .filter(|region| region.width() >= 1.0 && region.height() >= 1.0)
            .ok_or(Error::NothingSelected)?;
        let scale = self.scale(region);

        if let Some(only) = self.covered_by(region) {
            return only.cut(region);
        }

        let width = (region.width() * scale).round().max(1.0) as u32;
        let height = (region.height() * scale).round().max(1.0) as u32;
        let mut canvas = Image::filled(width, height, ColorType::Rgba8, Color::BLACK)?;

        for panel in &self.panels {
            let Some(part) = region.intersection(&panel.bounds) else {
                continue;
            };
            let piece = match panel.cut(part) {
                Ok(piece) => piece,
                Err(error) => {
                    tracing::warn!("output `{}` missing from screenshot: {error}", panel.name);
                    continue;
                }
            };
            let size = (
                (part.width() * scale).round().max(1.0) as u32,
                (part.height() * scale).round().max(1.0) as u32,
            );
            let piece = match piece.dimensions() == size {
                true => piece,
                false => piece.resize_exact(size.0, size.1, Filter::Lanczos3)?,
            };
            let x = ((part.min_x() - region.min_x()) * scale).round() as i64;
            let y = ((part.min_y() - region.min_y()) * scale).round() as i64;
            canvas.paste_in_place(&piece, x, y);
        }
        Ok(canvas)
    }
}

fn covered(panel: &Panel, region: Rect) -> f32 {
    panel
        .bounds
        .intersection(&region)
        .map_or(0.0, |part| part.width() * part.height())
}

fn capture(place: &Placement, cursor: bool) -> Result<Panel> {
    let source = match place.name.is_empty() {
        true => Source::FirstOutput,
        false => Source::Output(place.name.clone()),
    };
    let shot = Shot::take(&source, ShotOptions::default().cursor(cursor))?;
    let size = match place.size.width >= 1.0 && place.size.height >= 1.0 {
        true => place.size,
        false => Size::new(shot.width as f32, shot.height as f32),
    };
    Ok(Panel {
        name: place.name.clone(),
        bounds: Rect::from_origin_size(place.position, size),
        width: shot.width,
        height: shot.height,
        pixels: shot.pixels,
    })
}
