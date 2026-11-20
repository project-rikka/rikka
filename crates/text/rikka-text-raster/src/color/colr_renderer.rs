use rikka_text_font::{Bounds, ColorGlyph, Font, Location, OutlineScratch, Transform};

use rikka_simd::Level;
use rikka_vector::{BlendMode, Canvas, GLYPH_FLATNESS, Stop};

use crate::color::bounds_painter::{BoundsPainter, has_no_area};
use crate::color::colr_painter::{ColrPainter, Inks};
use crate::color::transform_stack::TransformStack;
use crate::image::{GlyphImage, GlyphImageKind, Placement};
use crate::outline::path::Outline;

const MAX_SIDE: f32 = 1024.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ColrJob<'p, 'f> {
    pub(crate) font: &'p Font<'f>,
    pub(crate) glyph: ColorGlyph<'f>,
    pub(crate) location: &'p Location,
    pub(crate) inks: Inks<'f>,
    pub(crate) device: Transform,
}

#[derive(Debug)]
pub(crate) struct ColrRenderer {
    canvas: Canvas,
    stops: Vec<Stop>,
    modes: Vec<BlendMode>,
    outline: Outline,
    transforms: TransformStack,
    clips: Vec<Bounds>,
}

impl ColrRenderer {
    pub(crate) fn with_level(level: Level) -> Self {
        Self {
            canvas: Canvas::with_level(level).with_flatness(GLYPH_FLATNESS),
            stops: Vec::new(),
            modes: Vec::new(),
            outline: Outline::default(),
            transforms: TransformStack::default(),
            clips: Vec::new(),
        }
    }

    pub(crate) fn render(
        &mut self,
        job: ColrJob<'_, '_>,
        scratch: &mut OutlineScratch,
        image: &mut GlyphImage,
    ) -> bool {
        let coords = job.location.coords();
        self.transforms.reset(job.device);
        self.clips.clear();
        let mut bounds = BoundsPainter {
            font: job.font,
            location: job.location,
            scratch,
            outline: &mut self.outline,
            transforms: &mut self.transforms,
            clips: &mut self.clips,
            total: None,
        };
        if job.glyph.paint(coords, &mut bounds).is_err() {
            return false;
        }
        let Some(total) = bounds.total.filter(|total| !has_no_area(total)) else {
            image.clear(GlyphImageKind::Color);
            return true;
        };
        let left = total.x_min.floor();
        let top = total.y_min.floor();
        let width = total.x_max.ceil() - left;
        let height = total.y_max.ceil() - top;
        let fits = width <= MAX_SIDE && height <= MAX_SIDE;
        if !fits {
            return false;
        }
        let placement = Placement {
            left: left as i32,
            top: -(top as i32),
            width: width as u32,
            height: height as u32,
        };
        self.canvas.begin(placement.width, placement.height);
        self.modes.clear();
        self.transforms
            .reset(job.device.then(Transform::translate(-left, -top)));
        let mut painter = ColrPainter {
            font: job.font,
            location: job.location,
            inks: job.inks,
            scratch,
            outline: &mut self.outline,
            canvas: &mut self.canvas,
            stops: &mut self.stops,
            modes: &mut self.modes,
            transforms: &mut self.transforms,
            failed: false,
        };
        if job.glyph.paint(coords, &mut painter).is_err() || painter.failed {
            return false;
        }
        image
            .prepare(GlyphImageKind::Color, placement)
            .copy_from_slice(self.canvas.pixels());
        true
    }
}
