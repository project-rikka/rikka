use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;

use rikka_text_catalog::catalog;
use rikka_text_font::GlyphId;
use rikka_text_raster::{GlyphImage, GlyphImageKind, GlyphRequest, Rasterizer, Rendered};
use rikka_types::Rect;
use rikka_vector::{
    Canvas, Composite, FillRule, Paint, Path, Pattern, Pixmap, Sampling, Tiling, Transform,
};

use crate::render::bounds::{node_bounds, scales};
use crate::render::{Host, covering, draw_node};
use crate::tree::{GlyphContent, GlyphPicture, RasterGlyph};

const MAX_PPEM: f32 = 512.0;
const MIN_PPEM: f32 = 0.25;
const KEPT: usize = 32;

thread_local! {
    static PAINTER: RefCell<Painter> = RefCell::new(Painter::default());
}

#[derive(Default)]
struct Painter {
    rasterizer: Rasterizer,
    image: GlyphImage,
    kept: VecDeque<(RasterGlyph, u32, Option<Arc<Drawn>>)>,
}

struct Drawn {
    pixmap: Pixmap,
    left: i32,
    top: i32,
}

pub(crate) fn draw(glyph: &GlyphPicture, parent: &Transform, host: Host, canvas: &mut Canvas) {
    let transform = glyph.transform.then(*parent);
    match &glyph.content {
        GlyphContent::Vector { node, placement } => {
            let placed = placement.then(transform);
            if glyph.opacity >= 1.0 {
                draw_node(node, &placed, host, canvas);
                return;
            }
            let Some(reach) = node_bounds(node, &placed, true) else {
                return;
            };
            canvas.push_layer_within(covering(reach));
            draw_node(node, &placed, host, canvas);
            canvas.pop_layer(&Composite::faded(glyph.opacity));
        }
        GlyphContent::Raster(raster) => {
            let stretch = scales(&transform);
            let ppem = raster.size * stretch.x.max(stretch.y);
            if ppem.is_nan() || ppem < MIN_PPEM {
                return;
            }
            let ppem = ppem.min(MAX_PPEM);
            let Some(drawn) = PAINTER.with(|painter| painter.borrow_mut().drawn(raster, ppem))
            else {
                return;
            };
            let density = ppem / raster.size;
            let placement = Transform::translate(drawn.left as f32, -drawn.top as f32)
                .then(Transform::scale(1.0 / density, 1.0 / density))
                .then(transform);
            let mut window = Path::new();
            window.add_rect(Rect::new(
                0.0,
                0.0,
                drawn.pixmap.width() as f32,
                drawn.pixmap.height() as f32,
            ));
            let paint = Paint::Pattern(Pattern {
                pixmap: &drawn.pixmap,
                transform: Transform::IDENTITY,
                tiling: Tiling::Once,
                sampling: Sampling::Bilinear,
                opacity: glyph.opacity,
            });
            if let Err(error) = canvas.fill(&window, &placement, FillRule::NonZero, &paint) {
                tracing::debug!("skipping SVG colour glyph: {error}");
            }
        }
    }
}

impl Painter {
    fn drawn(&mut self, raster: &RasterGlyph, ppem: f32) -> Option<Arc<Drawn>> {
        let key = ppem.to_bits();
        if let Some(at) = self
            .kept
            .iter()
            .position(|(glyph, size, _)| glyph == raster && *size == key)
        {
            let entry = self.kept.remove(at)?;
            self.kept.push_front(entry);
            return self.kept[0].2.clone();
        }
        let drawn = self.render(raster, ppem).map(Arc::new);
        self.kept.truncate(KEPT - 1);
        self.kept.push_front((*raster, key, drawn.clone()));
        drawn
    }

    fn render(&mut self, raster: &RasterGlyph, ppem: f32) -> Option<Drawn> {
        let font = catalog().font(raster.face)?;
        let request = GlyphRequest::new(GlyphId(raster.id), ppem)
            .location(raster.location.location())
            .synthesis(raster.synthesis)
            .foreground(raster.foreground)
            .color(true);
        match self.rasterizer.render(&font, &request, &mut self.image) {
            Ok(Rendered::Image) => {}
            Ok(Rendered::Svg(_)) => return None,
            Err(error) => {
                tracing::debug!("skipping SVG colour glyph {}: {error}", raster.id);
                return None;
            }
        }
        let image = &self.image;
        if image.is_empty() {
            return None;
        }
        let pixels = match image.kind() {
            GlyphImageKind::Color => image.pixels().to_vec(),
            GlyphImageKind::Mask => {
                let ink = raster.foreground;
                image
                    .pixels()
                    .iter()
                    .flat_map(|coverage| {
                        let alpha = u32::from(*coverage) * u32::from(ink.a) / 255;
                        let scaled = |channel: u8| ((u32::from(channel) * alpha + 127) / 255) as u8;
                        [scaled(ink.r), scaled(ink.g), scaled(ink.b), alpha as u8]
                    })
                    .collect()
            }
        };
        let pixmap = Pixmap::from_premultiplied(image.width(), image.height(), pixels).ok()?;
        Some(Drawn {
            pixmap,
            left: image.left(),
            top: image.top(),
        })
    }
}
