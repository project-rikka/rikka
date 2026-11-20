use rikka_svg::Svg;
use rikka_text_catalog::{FaceId, catalog};
use rikka_text_font::{Font, GlyphId, OutlineSink, Transform};
use rikka_text_layout::{Glyph, LocationId};
use rikka_text_raster::{CurveBlob, CurveHeader, GlyphRequest, OutlineCache, Rasterizer, Rendered};

use crate::glyph_image::{GlyphImage, GlyphKind};
use crate::glyph_key::{CurveKey, GlyphKey};
use crate::icon_key::IconKey;
use crate::icon_raster::icon_glyph;
use crate::svg_documents::{SvgDocumentKey, SvgDocuments};

type OutlineKey = (FaceId, LocationId, u16);

#[derive(Default)]
pub struct GlyphRaster {
    rasterizer: Rasterizer,
    image: rikka_text_raster::GlyphImage,
    outlines: OutlineCache<OutlineKey>,
    svgs: SvgDocuments,
}

impl GlyphRaster {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn raster(&mut self, key: GlyphKey) -> Option<GlyphImage> {
        let font = catalog().font(key.face())?;
        if !key.is_color() && font.outline_format().is_some() {
            return self.raster_outline(&font, key);
        }
        let svg = key
            .is_color()
            .then(|| svg_document_of(&font, key))
            .flatten();
        if let Some(document) = svg
            && let Some(cached) = self.svgs.cached(document)
        {
            return svg_image(cached?, &font, key);
        }
        match self
            .rasterizer
            .render(&font, &key.request(), &mut self.image)
            .ok()?
        {
            Rendered::Image => GlyphImage::from_raster(&self.image),
            Rendered::Svg(rendered) => {
                let document = SvgDocumentKey::for_glyph(
                    &font,
                    key.face(),
                    rendered.glyph,
                    key.color_scheme(),
                )?;
                svg_image(self.svgs.parsed(document, rendered.document)?, &font, key)
            }
        }
    }

    fn raster_outline(&mut self, font: &Font<'_>, key: GlyphKey) -> Option<GlyphImage> {
        let Self {
            rasterizer,
            image,
            outlines,
            ..
        } = self;
        let request = key.request();
        let outline = outlines
            .outline((key.face(), key.location(), key.glyph()), |outline| {
                rasterizer.prepare_outline(
                    font,
                    request.glyph(),
                    key.location().location(),
                    outline,
                )
            })
            .ok()?;
        rasterizer.render_prepared(outline, &request, image).ok()?;
        GlyphImage::from_raster(image)
    }

    pub fn outline(
        &mut self,
        glyph: &Glyph,
        origin: (f32, f32),
        scale: f32,
        path: &mut impl OutlineSink,
    ) -> bool {
        if glyph.color {
            return false;
        }
        let Some(font) = catalog().font(glyph.face) else {
            return false;
        };
        if font.outline_format().is_none() {
            return false;
        }
        let Self {
            rasterizer,
            outlines,
            ..
        } = self;
        let location = glyph.location.location();
        let Ok(outline) = outlines.outline((glyph.face, glyph.location, glyph.id), |outline| {
            rasterizer.prepare_outline(&font, GlyphId(glyph.id), location, outline)
        }) else {
            return false;
        };
        let pen = Transform {
            yy: -1.0,
            dx: origin.0 + glyph.x * scale,
            dy: origin.1 + glyph.y * scale,
            ..Transform::IDENTITY
        };
        outline
            .trace(glyph.synthesis, glyph.font_size * scale, &pen, path)
            .is_ok()
    }

    pub fn raster_icon(&mut self, key: IconKey) -> Option<GlyphImage> {
        let icon = icon_glyph(key)?;
        let (right, rise) = icon.pen;
        let request = GlyphRequest::new(icon.glyph, key.size())
            .location(&icon.location)
            .subpixel_x(right)
            .subpixel_y(rise)
            .color(false);
        self.rasterizer
            .render_outline(&icon.font, &request, &mut self.image)
            .ok()?;
        let mut image = GlyphImage::from_raster(&self.image)?;
        image.left += right.floor() as i32;
        image.top += rise.floor() as i32;
        Some(image)
    }

    pub fn curves(&mut self, key: CurveKey, blob: &mut CurveBlob) -> Option<CurveHeader> {
        let font = catalog().font(key.face())?;
        let request = key.request(f32::from(font.units_per_em()));
        self.rasterizer.encode_curves(&font, &request, blob).ok()
    }
}

fn svg_document_of(font: &Font<'_>, key: GlyphKey) -> Option<SvgDocumentKey> {
    let glyph = GlyphId(key.glyph());
    if glyph.0 >= font.glyph_count() || font.color_glyph(glyph).is_some() {
        return None;
    }
    SvgDocumentKey::for_glyph(font, key.face(), glyph, key.color_scheme())
}

fn svg_image(svg: &Svg, font: &Font<'_>, key: GlyphKey) -> Option<GlyphImage> {
    let scale = key.size() / f32::from(font.units_per_em().max(1));
    match svg.render_glyph(key.glyph(), scale, key.subpixel_x()) {
        Ok(pixels) => pixels.map(|pixels| GlyphImage {
            kind: GlyphKind::Color,
            left: pixels.left,
            top: pixels.top,
            width: pixels.width,
            height: pixels.height,
            pixels: pixels.premultiplied,
        }),
        Err(error) => {
            tracing::warn!("failed to render SVG glyph: {error}");
            None
        }
    }
}

impl std::fmt::Debug for GlyphRaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphRaster")
            .field("level", &self.rasterizer.level())
            .finish_non_exhaustive()
    }
}
