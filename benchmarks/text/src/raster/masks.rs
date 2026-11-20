use rikka_text_font::{Font, GlyphId, Location};
use rikka_text_raster::{GlyphImage, GlyphRequest, PreparedOutline, Rasterizer};
use swash::scale::{Render, ScaleContext, Source, image::Image};
use swash::zeno::{Format, Vector};

use text_parsers::codepoints;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mask {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Difference {
    pub total: f64,
    pub pixels: usize,
    pub max: u8,
}

impl Difference {
    pub fn add(&mut self, other: Difference) {
        self.total += other.total;
        self.pixels += other.pixels;
        self.max = self.max.max(other.max);
    }

    pub fn mean(&self) -> f64 {
        if self.pixels == 0 {
            return 0.0;
        }
        self.total / self.pixels as f64
    }
}

impl Mask {
    fn coverage(&self, x: i32, y: i32) -> u8 {
        let column = x - self.left;
        let row = self.top - y;
        if column < 0 || row < 0 || column >= self.width as i32 || row >= self.height as i32 {
            return 0;
        }
        self.pixels[row as usize * self.width as usize + column as usize]
    }

    fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn difference(&self, other: &Mask) -> Difference {
        let boxes = [self, other].into_iter().filter(|mask| !mask.is_empty());
        let (mut left, mut right, mut bottom, mut top) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for mask in boxes {
            left = left.min(mask.left);
            right = right.max(mask.left + mask.width as i32);
            top = top.max(mask.top);
            bottom = bottom.min(mask.top - mask.height as i32);
        }
        let mut difference = Difference::default();
        for y in (bottom + 1..=top).rev() {
            for x in left..right {
                let delta = self.coverage(x, y).abs_diff(other.coverage(x, y));
                difference.total += f64::from(delta) / 255.0;
                difference.pixels += 1;
                difference.max = difference.max.max(delta);
            }
        }
        difference
    }
}

pub fn raster_glyphs(font: &Font<'_>) -> Vec<GlyphId> {
    let Some(charmap) = font.charmap() else {
        return Vec::new();
    };
    let mut glyphs: Vec<GlyphId> = codepoints()
        .into_iter()
        .filter_map(|code| charmap.map(code))
        .filter(|glyph| glyph.0 != 0)
        .collect();
    glyphs.sort();
    glyphs.dedup();
    glyphs
}

pub fn subpixel_offset(index: usize) -> f32 {
    (index % 4) as f32 * 0.25
}

#[derive(Debug, Default)]
pub struct RikkaMasks {
    rasterizer: Rasterizer,
    image: GlyphImage,
    location: Location,
}

impl RikkaMasks {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn location(mut self, location: Location) -> Self {
        self.location = location;
        self
    }

    pub fn render(&mut self, font: &Font<'_>, glyph: GlyphId, ppem: f32, offset: f32) -> usize {
        let request = GlyphRequest::new(glyph, ppem)
            .subpixel_x(offset)
            .location(&self.location);
        match self
            .rasterizer
            .render_outline(font, &request, &mut self.image)
        {
            Ok(()) => self.image.pixels().len(),
            Err(_) => 0,
        }
    }

    pub fn mask(&mut self, font: &Font<'_>, glyph: GlyphId, ppem: f32, offset: f32) -> Mask {
        self.render(font, glyph, ppem, offset);
        Mask {
            left: self.image.left(),
            top: self.image.top(),
            width: self.image.width(),
            height: self.image.height(),
            pixels: self.image.pixels().to_vec(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PreparedMasks {
    rasterizer: Rasterizer,
    image: GlyphImage,
    outlines: Vec<rikka_text_raster::Result<PreparedOutline>>,
}

impl PreparedMasks {
    pub fn new(font: &Font<'_>, glyphs: &[GlyphId], location: Location) -> Self {
        let mut rasterizer = Rasterizer::new();
        let outlines = glyphs
            .iter()
            .map(|&glyph| {
                let mut outline = PreparedOutline::new();
                rasterizer
                    .prepare_outline(font, glyph, &location, &mut outline)
                    .map(|()| outline)
            })
            .collect();
        Self {
            rasterizer,
            image: GlyphImage::new(),
            outlines,
        }
    }

    pub fn render_all(&mut self, glyphs: &[GlyphId], ppem: f32) -> usize {
        let mut pixels = 0;
        for (index, (outline, &glyph)) in self.outlines.iter_mut().zip(glyphs).enumerate() {
            let request = GlyphRequest::new(glyph, ppem).subpixel_x(subpixel_offset(index));
            if let Ok(outline) = outline
                && self
                    .rasterizer
                    .render_prepared(outline, &request, &mut self.image)
                    .is_ok()
            {
                pixels += self.image.pixels().len();
            }
        }
        pixels
    }
}

pub struct SwashMasks {
    context: ScaleContext,
    image: Image,
    weight: Option<f32>,
}

impl Default for SwashMasks {
    fn default() -> Self {
        Self::new()
    }
}

impl SwashMasks {
    pub fn new() -> Self {
        Self {
            context: ScaleContext::new(),
            image: Image::new(),
            weight: None,
        }
    }

    #[must_use]
    pub fn weight(mut self, weight: Option<f32>) -> Self {
        self.weight = weight;
        self
    }

    pub fn render_all(
        &mut self,
        font: swash::FontRef<'_>,
        glyphs: &[GlyphId],
        ppem: f32,
        mut each: impl FnMut(usize, &Image),
    ) {
        let mut scaler = self
            .context
            .builder(font)
            .size(ppem)
            .hint(false)
            .variations(self.weight.map(|weight| ("wght", weight)))
            .build();
        for (index, glyph) in glyphs.iter().enumerate() {
            self.image.data.clear();
            Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .offset(Vector::new(subpixel_offset(index), 0.0))
                .render_into(&mut scaler, glyph.0, &mut self.image);
            each(index, &self.image);
        }
    }

    pub fn mask(
        &mut self,
        font: swash::FontRef<'_>,
        glyph: GlyphId,
        ppem: f32,
        offset: f32,
    ) -> Mask {
        let mut scaler = self
            .context
            .builder(font)
            .size(ppem)
            .hint(false)
            .variations(self.weight.map(|weight| ("wght", weight)))
            .build();
        self.image.data.clear();
        Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(offset, 0.0))
            .render_into(&mut scaler, glyph.0, &mut self.image);
        let placement = self.image.placement;
        Mask {
            left: placement.left,
            top: placement.top,
            width: placement.width,
            height: placement.height,
            pixels: self.image.data.clone(),
        }
    }
}

pub fn fontdue_mask(font: &fontdue::Font, glyph: GlyphId, ppem: f32) -> Mask {
    let (metrics, pixels) = font.rasterize_indexed(glyph.0, ppem);
    Mask {
        left: metrics.xmin,
        top: metrics.ymin + metrics.height as i32,
        width: metrics.width as u32,
        height: metrics.height as u32,
        pixels,
    }
}

impl std::fmt::Debug for SwashMasks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwashMasks").finish_non_exhaustive()
    }
}
