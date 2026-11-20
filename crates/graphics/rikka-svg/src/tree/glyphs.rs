use std::collections::HashMap;
use std::sync::Arc;

use rikka_text_catalog::catalog;
use rikka_text_font::{
    Color as GlyphColor, Font, GlyphId, OutlineSink, Tag, Transform as GlyphTransform,
};
use rikka_text_layout::{FaceId, Glyph, LocationId};
use rikka_text_raster::{OutlineCache, Rasterizer};
use rikka_types::{Point, Rect};
use rikka_vector::{Path, Transform};

use crate::document::{Loader, Svg};
use crate::resources::SvgGlyphs;
use crate::tree::{GlyphContent, GlyphPicture, Node, RasterGlyph};

const COLOR_BITMAPS: Tag = Tag::new(b"CBDT");
const APPLE_BITMAPS: Tag = Tag::new(b"sbix");
const KEPT_DOCUMENTS: usize = 8;

pub(crate) struct Glyphs {
    rasterizer: Rasterizer,
    outlines: OutlineCache<(FaceId, LocationId, u16)>,
    svg_glyphs: SvgGlyphs,
    documents: HashMap<(FaceId, u16), Option<Svg>>,
    pictures: HashMap<(FaceId, u16), Option<DocumentGlyph>>,
    pub(crate) parsed_bytes: usize,
    pub(crate) follows_scheme: bool,
}

#[derive(Clone)]
struct DocumentGlyph {
    node: Arc<Node>,
    parent: Transform,
}

pub(crate) struct GlyphInk {
    pub(crate) foreground: GlyphColor,
    pub(crate) opacity: f32,
}

impl Glyphs {
    pub(crate) fn new(svg_glyphs: SvgGlyphs) -> Glyphs {
        Glyphs {
            rasterizer: Rasterizer::new(),
            outlines: OutlineCache::default(),
            svg_glyphs,
            documents: HashMap::new(),
            pictures: HashMap::new(),
            parsed_bytes: 0,
            follows_scheme: false,
        }
    }

    pub(crate) fn place(
        &mut self,
        glyph: &Glyph,
        transform: &Transform,
        ink: Option<&GlyphInk>,
        outlines: &mut Path,
        pictures: &mut Vec<GlyphPicture>,
    ) {
        let Some(font) = catalog().font(glyph.face) else {
            return;
        };
        let id = GlyphId(glyph.id);
        if !is_colored(glyph, &font, id) {
            self.trace(&font, glyph, transform, outlines);
            return;
        }
        let Some(ink) = ink else {
            return;
        };
        let vector = (self.svg_glyphs == SvgGlyphs::Embedded
            && font.color_glyph(id).is_none()
            && font.svg_document(id).is_some())
        .then(|| self.picture(&font, glyph.face, id))
        .flatten();
        let content = match vector {
            Some(DocumentGlyph { node, parent }) => {
                let scale = glyph.font_size / f32::from(font.units_per_em().max(1));
                GlyphContent::Vector {
                    node,
                    placement: parent.then(Transform::scale(scale, scale)),
                }
            }
            None => GlyphContent::Raster(RasterGlyph {
                face: glyph.face,
                location: glyph.location,
                id: glyph.id,
                size: glyph.font_size,
                synthesis: glyph.synthesis,
                foreground: ink.foreground,
            }),
        };
        let extent = match &content {
            GlyphContent::Vector { node, placement } => {
                crate::render::bounds::node_bounds(node, placement, true)
            }
            GlyphContent::Raster(_) => None,
        }
        .unwrap_or_else(|| face_extent(&font, glyph));
        pictures.push(GlyphPicture {
            transform: *transform,
            extent,
            opacity: ink.opacity,
            content,
        });
    }

    fn trace(&mut self, font: &Font<'_>, glyph: &Glyph, transform: &Transform, path: &mut Path) {
        let Self {
            rasterizer,
            outlines,
            ..
        } = self;
        let location = glyph.location.location();
        let Ok(outline) = outlines.outline((glyph.face, glyph.location, glyph.id), |outline| {
            rasterizer.prepare_outline(font, GlyphId(glyph.id), location, outline)
        }) else {
            return;
        };
        let device = GlyphTransform {
            xx: transform.xx,
            yx: transform.yx,
            xy: -transform.xy,
            yy: -transform.yy,
            dx: transform.dx,
            dy: transform.dy,
        };
        let mut sink = Sink { path };
        if let Err(error) = outline.trace(glyph.synthesis, glyph.font_size, &device, &mut sink) {
            tracing::debug!("skipping SVG glyph outline {}: {error}", glyph.id);
        }
    }

    fn picture(&mut self, font: &Font<'_>, face: FaceId, glyph: GlyphId) -> Option<DocumentGlyph> {
        if let Some(known) = self.pictures.get(&(face, glyph.0)) {
            return known.clone();
        }
        let document = font.svg_document(glyph)?;
        let key = (face, document.first_glyph.0);
        if !self.documents.contains_key(&key) {
            if self.documents.len() == KEPT_DOCUMENTS {
                self.documents.clear();
            }
            self.parsed_bytes += document.data.len();
            let parsed = Loader::default()
                .svg_glyphs(SvgGlyphs::Skipped)
                .from_bytes(document.data)
                .inspect_err(|error| tracing::debug!("skipping SVG glyph document: {error}"))
                .ok();
            self.follows_scheme |= parsed.as_ref().is_some_and(Svg::follows_color_scheme);
            self.documents.insert(key, parsed);
        }
        let found = self.documents[&key].as_ref().map(|svg| {
            let (node, parent) = svg.glyph_node(glyph.0);
            DocumentGlyph {
                node: Arc::new(node.into_owned()),
                parent,
            }
        });
        self.pictures.insert((face, glyph.0), found.clone());
        found
    }
}

fn is_colored(glyph: &Glyph, font: &Font<'_>, id: GlyphId) -> bool {
    font.outline_format().is_none()
        || (glyph.color
            && (font.color_glyph(id).is_some()
                || font.svg_document(id).is_some()
                || font.has_table(COLOR_BITMAPS)
                || font.has_table(APPLE_BITMAPS)))
}

fn face_extent(font: &Font<'_>, glyph: &Glyph) -> Rect {
    let metrics = font.metrics(glyph.location.location());
    let scale = glyph.font_size / f32::from(metrics.units_per_em.max(1));
    let bounds = metrics.bounds;
    let (width, height) = (bounds.x_max - bounds.x_min, bounds.y_max - bounds.y_min);
    match width > 0.0 && height > 0.0 {
        true => Rect::new(
            bounds.x_min * scale,
            -bounds.y_max * scale,
            width * scale,
            height * scale,
        ),
        false => Rect::new(
            0.0,
            -glyph.font_size,
            glyph.font_size,
            glyph.font_size * 1.25,
        ),
    }
}

struct Sink<'p> {
    path: &'p mut Path,
}

impl OutlineSink for Sink<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(Point::new(x1, y1), Point::new(x, y));
    }

    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path
            .cubic_to(Point::new(x1, y1), Point::new(x2, y2), Point::new(x, y));
    }

    fn close(&mut self) {
        self.path.close();
    }
}
