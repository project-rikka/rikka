use rikka_simd::Level;
use rikka_text_font::{
    ColorGlyphPaint, Font, GlyphId, Location, OutlineScratch, Point, Tag, Transform,
};

use crate::bitmap::bitmap_renderer::{BitmapRenderer, BitmapTarget};
use crate::color::colr_painter::Inks;
use crate::color::colr_renderer::{ColrJob, ColrRenderer};
use crate::curves::blob::{CurveBlob, CurveHeader};
use crate::curves::encoder::CurveEncoder;
use crate::distortion::Distortion;
use crate::error::{Error, Result};
use crate::image::GlyphImage;
use crate::mask::MaskRasterizer;
use crate::outline::path::Outline;
use crate::outline::prepared::PreparedOutline;
use crate::request::GlyphRequest;
use crate::svg_glyph::{SvgExtractor, SvgGlyph};

const COLOR_BITMAPS: Tag = Tag::new(b"CBDT");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rendered<'r> {
    Image,
    Svg(SvgGlyph<'r>),
}

#[derive(Debug)]
pub struct Rasterizer {
    masks: MaskRasterizer,
    prepared: PreparedOutline,
    scratch: OutlineScratch,
    default_location: Location,
    colr: ColrRenderer,
    bitmaps: BitmapRenderer,
    svg: SvgExtractor,
    curves: CurveEncoder,
}

impl Default for Rasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Rasterizer {
    pub fn new() -> Self {
        Self::with_level(Level::detect())
    }

    pub fn with_level(level: Level) -> Self {
        Self {
            masks: MaskRasterizer::new(level),
            prepared: PreparedOutline::new(),
            scratch: OutlineScratch::new(),
            default_location: Location::default(),
            colr: ColrRenderer::with_level(level),
            bitmaps: BitmapRenderer::default(),
            svg: SvgExtractor::default(),
            curves: CurveEncoder::default(),
        }
    }

    #[inline]
    pub fn level(&self) -> Level {
        self.masks.level()
    }

    pub fn render<'r, 'f: 'r>(
        &'r mut self,
        font: &Font<'f>,
        request: &GlyphRequest<'_>,
        image: &mut GlyphImage,
    ) -> Result<Rendered<'r>> {
        if !request.is_valid() {
            return Err(Error::InvalidRequest);
        }
        let count = font.glyph_count();
        if request.glyph.0 >= count {
            return Err(Error::Font(rikka_text_font::Error::GlyphIndex {
                glyph: request.glyph.0,
                count,
            }));
        }
        if request.color {
            let color_glyph = font.color_glyph(request.glyph);
            let graph = color_glyph
                .is_some_and(|glyph| matches!(glyph.paint_root(), ColorGlyphPaint::Graph(_)));
            if graph && self.render_colr(font, request, image) {
                return Ok(Rendered::Image);
            }
            if font.svg_document(request.glyph).is_some() {
                return self
                    .svg
                    .extract(font, request.glyph)
                    .map(Rendered::Svg)
                    .ok_or(Error::MalformedSvg);
            }
            if color_glyph.is_some() && !graph && self.render_colr(font, request, image) {
                return Ok(Rendered::Image);
            }
            let target = bitmap_target(request);
            if font.has_table(COLOR_BITMAPS)
                && self
                    .bitmaps
                    .render_strike(font, request.glyph, target, image)
            {
                return Ok(Rendered::Image);
            }
            if self.bitmaps.render_sbix(font, request.glyph, target, image) {
                return Ok(Rendered::Image);
            }
        }
        if font.outline_format().is_some() {
            return self
                .render_outline(font, request, image)
                .map(|()| Rendered::Image);
        }
        self.bitmaps
            .render_strike(font, request.glyph, bitmap_target(request), image)
            .then_some(Rendered::Image)
            .ok_or(Error::NoImage)
    }

    pub fn render_outline(
        &mut self,
        font: &Font<'_>,
        request: &GlyphRequest<'_>,
        image: &mut GlyphImage,
    ) -> Result<()> {
        if !request.is_valid() {
            return Err(Error::InvalidRequest);
        }
        let location = request.location.unwrap_or(&self.default_location);
        self.prepared
            .load(font, request.glyph, location, &mut self.scratch)?;
        fill_prepared(&mut self.masks, &mut self.prepared, request, image)
    }

    pub fn prepare_outline(
        &mut self,
        font: &Font<'_>,
        glyph: GlyphId,
        location: &Location,
        prepared: &mut PreparedOutline,
    ) -> Result<()> {
        prepared.load(font, glyph, location, &mut self.scratch)
    }

    pub fn render_prepared(
        &mut self,
        prepared: &mut PreparedOutline,
        request: &GlyphRequest<'_>,
        image: &mut GlyphImage,
    ) -> Result<()> {
        fill_prepared(&mut self.masks, prepared, request, image)
    }

    pub fn fill(
        &mut self,
        outline: &Outline,
        transform: &Transform,
        image: &mut GlyphImage,
    ) -> Result<()> {
        self.masks
            .fill(outline, transform, image)
            .then_some(())
            .ok_or(Error::TooComplex)
    }

    pub fn encode_curves(
        &mut self,
        font: &Font<'_>,
        request: &GlyphRequest<'_>,
        blob: &mut CurveBlob,
    ) -> Result<CurveHeader> {
        blob.words.clear();
        let units_per_em = f32::from(font.units_per_em());
        let distortion = Distortion::new(request.synthesis, units_per_em);
        let location = request.location.unwrap_or(&self.default_location);
        self.prepared
            .load(font, request.glyph, location, &mut self.scratch)?;
        self.prepared.embolden_in_place(distortion.embolden);
        let to_em = distortion.placement(units_per_em.recip(), Point::default());
        if !self.curves.encode(&self.prepared.outline, &to_em, blob) {
            blob.words.clear();
            return Err(Error::TooComplex);
        }
        blob.header().ok_or(Error::TooComplex)
    }

    fn render_colr(
        &mut self,
        font: &Font<'_>,
        request: &GlyphRequest<'_>,
        image: &mut GlyphImage,
    ) -> bool {
        let Some(glyph) = font.color_glyph(request.glyph) else {
            return false;
        };
        let units_per_em = f32::from(font.units_per_em());
        let scale = request.ppem / units_per_em;
        let skew = Distortion::new(request.synthesis, units_per_em).skew;
        let palette = font
            .cpal()
            .and_then(|cpal| cpal.palette(request.palette).or_else(|| cpal.palette(0)));
        let job = ColrJob {
            font,
            glyph,
            location: request.location.unwrap_or(&self.default_location),
            inks: Inks {
                palette,
                foreground: request.foreground,
            },
            device: Transform {
                xx: scale,
                yx: 0.0,
                xy: scale * skew,
                yy: -scale,
                dx: request.subpixel_x,
                dy: -request.subpixel_y,
            },
        };
        self.colr.render(job, &mut self.scratch, image)
    }
}

fn bitmap_target(request: &GlyphRequest<'_>) -> BitmapTarget {
    BitmapTarget {
        ppem: request.ppem,
        subpixel_x: request.subpixel_x,
        subpixel_y: request.subpixel_y,
    }
}

fn fill_prepared(
    masks: &mut MaskRasterizer,
    prepared: &mut PreparedOutline,
    request: &GlyphRequest<'_>,
    image: &mut GlyphImage,
) -> Result<()> {
    if !request.is_valid() {
        return Err(Error::InvalidRequest);
    }
    if !prepared.is_loaded() {
        return Err(Error::NotLoaded);
    }
    let units_per_em = prepared.units_per_em;
    let distortion = Distortion::new(request.synthesis, units_per_em);
    let scale = request.ppem / units_per_em;
    let transform = distortion.placement(scale, Point::new(request.subpixel_x, request.subpixel_y));
    masks
        .fill_prepared(prepared, &transform, distortion.embolden, image)
        .then_some(())
        .ok_or(Error::TooComplex)
}
