use crate::color::color_line::{ColorStops, Extend};
use crate::color::color_value::ColorIndex;
use crate::color::composite_mode::CompositeMode;
use crate::color::transform::Transform;
use crate::outline::bounds::Bounds;
use crate::sfnt::glyph_id::GlyphId;
use rikka_types::Point;

#[derive(Clone, Copy, Debug)]
pub enum Brush<'c> {
    Solid {
        color: ColorIndex,
        alpha: f32,
    },
    LinearGradient {
        start: Point,
        end: Point,
        stops: ColorStops<'c>,
        extend: Extend,
    },
    RadialGradient {
        start_center: Point,
        start_radius: f32,
        end_center: Point,
        end_radius: f32,
        stops: ColorStops<'c>,
        extend: Extend,
    },
    SweepGradient {
        center: Point,
        start_degrees: f32,
        end_degrees: f32,
        stops: ColorStops<'c>,
        extend: Extend,
    },
}

pub trait ColorPainter {
    fn push_transform(&mut self, transform: Transform);
    fn pop_transform(&mut self);
    fn push_clip_glyph(&mut self, glyph: GlyphId);
    fn push_clip_box(&mut self, clip: Bounds);
    fn pop_clip(&mut self);
    fn fill(&mut self, brush: Brush<'_>);
    fn push_layer(&mut self, mode: CompositeMode);
    fn pop_layer(&mut self);
}
