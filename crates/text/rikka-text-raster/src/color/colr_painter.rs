use rikka_text_font::{
    Bounds, Brush, Color, ColorIndex, ColorPainter, ColorStops, CompositeMode, Extend, Font,
    GlyphId, Location, OutlineScratch, OutlineSink, Palette, Transform,
};
use rikka_vector::{
    BlendMode, Canvas, Composite, Contours, FillRule, Gradient, GradientShape, Interpolation,
    Paint, Spread, Stop,
};

use crate::color::transform_stack::TransformStack;
use crate::device::device;
use crate::outline::path::Outline;

const TRANSPARENT: rikka_types::Color = rikka_types::Color::rgba(0.0, 0.0, 0.0, 0.0);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Inks<'a> {
    pub(crate) palette: Option<Palette<'a>>,
    pub(crate) foreground: Color,
}

impl Inks<'_> {
    fn ink(&self, color: ColorIndex, alpha: f32) -> Option<rikka_types::Color> {
        let color = match color {
            ColorIndex::Foreground => Some(self.foreground),
            ColorIndex::Palette(entry) => self.palette.and_then(|palette| palette.color(entry)),
        }?;
        Some(rikka_types::Color::rgba(
            f32::from(color.r) / 255.0,
            f32::from(color.g) / 255.0,
            f32::from(color.b) / 255.0,
            (f32::from(color.a) / 255.0 * alpha).clamp(0.0, 1.0),
        ))
    }
}

pub(crate) struct ColrPainter<'p, 'f> {
    pub(crate) font: &'p Font<'f>,
    pub(crate) location: &'p Location,
    pub(crate) inks: Inks<'f>,
    pub(crate) scratch: &'p mut OutlineScratch,
    pub(crate) outline: &'p mut Outline,
    pub(crate) canvas: &'p mut Canvas,
    pub(crate) stops: &'p mut Vec<Stop>,
    pub(crate) modes: &'p mut Vec<BlendMode>,
    pub(crate) transforms: &'p mut TransformStack,
    pub(crate) failed: bool,
}

impl ColrPainter<'_, '_> {
    fn clip_to_outline(&mut self) {
        let placed = device(&self.transforms.current());
        let clipped = self.canvas.push_clip(
            self.outline.path(),
            &placed,
            FillRule::NonZero,
            Contours::Simple,
        );
        if clipped.is_err() {
            self.failed = true;
        }
    }

    fn gradient(&mut self, shape: GradientShape, stops: ColorStops<'_>, extend: Extend) {
        let inks = self.inks;
        self.stops.clear();
        self.stops.extend(stops.iter().map(|stop| Stop {
            offset: stop.offset,
            color: inks.ink(stop.color, stop.alpha).unwrap_or(TRANSPARENT),
        }));
        let gradient = Gradient {
            shape,
            stops: self.stops,
            spread: match extend {
                Extend::Pad => Spread::Pad,
                Extend::Repeat => Spread::Repeat,
                Extend::Reflect => Spread::Reflect,
            },
            interpolation: Interpolation::LinearLight,
            transform: rikka_vector::Transform::IDENTITY,
        };
        self.canvas.paint(
            &Paint::Gradient(gradient),
            &device(&self.transforms.current()),
        );
    }
}

impl ColorPainter for ColrPainter<'_, '_> {
    fn push_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }

    fn push_clip_glyph(&mut self, glyph: GlyphId) {
        self.outline.clear();
        if self
            .font
            .outline(glyph, self.location, self.scratch, self.outline)
            .is_err()
        {
            self.outline.clear();
        }
        self.clip_to_outline();
    }

    fn push_clip_box(&mut self, clip: Bounds) {
        self.outline.clear();
        self.outline.move_to(clip.x_min, clip.y_min);
        self.outline.line_to(clip.x_max, clip.y_min);
        self.outline.line_to(clip.x_max, clip.y_max);
        self.outline.line_to(clip.x_min, clip.y_max);
        self.clip_to_outline();
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn fill(&mut self, brush: Brush<'_>) {
        match brush {
            Brush::Solid { color, alpha } => {
                if let Some(color) = self.inks.ink(color, alpha) {
                    self.canvas
                        .paint(&Paint::Solid(color), &rikka_vector::Transform::IDENTITY);
                }
            }
            Brush::LinearGradient {
                start,
                end,
                stops,
                extend,
            } => self.gradient(GradientShape::Linear { start, end }, stops, extend),
            Brush::RadialGradient {
                start_center,
                start_radius,
                end_center,
                end_radius,
                stops,
                extend,
            } => self.gradient(
                GradientShape::Radial {
                    start_center,
                    start_radius,
                    end_center,
                    end_radius,
                },
                stops,
                extend,
            ),
            Brush::SweepGradient {
                center,
                start_degrees,
                end_degrees,
                stops,
                extend,
            } => self.gradient(
                GradientShape::Sweep {
                    center,
                    start_degrees,
                    end_degrees,
                },
                stops,
                extend,
            ),
        }
    }

    fn push_layer(&mut self, mode: CompositeMode) {
        self.canvas.push_layer();
        self.modes.push(blend_mode(mode));
    }

    fn pop_layer(&mut self) {
        if let Some(mode) = self.modes.pop() {
            self.canvas.pop_layer(&Composite::blending(mode));
        }
    }
}

fn blend_mode(mode: CompositeMode) -> BlendMode {
    match mode {
        CompositeMode::Clear => BlendMode::Clear,
        CompositeMode::Source => BlendMode::Source,
        CompositeMode::Destination => BlendMode::Destination,
        CompositeMode::SourceOver => BlendMode::SourceOver,
        CompositeMode::DestinationOver => BlendMode::DestinationOver,
        CompositeMode::SourceIn => BlendMode::SourceIn,
        CompositeMode::DestinationIn => BlendMode::DestinationIn,
        CompositeMode::SourceOut => BlendMode::SourceOut,
        CompositeMode::DestinationOut => BlendMode::DestinationOut,
        CompositeMode::SourceAtop => BlendMode::SourceAtop,
        CompositeMode::DestinationAtop => BlendMode::DestinationAtop,
        CompositeMode::Xor => BlendMode::Xor,
        CompositeMode::Plus => BlendMode::Plus,
        CompositeMode::Screen => BlendMode::Screen,
        CompositeMode::Overlay => BlendMode::Overlay,
        CompositeMode::Darken => BlendMode::Darken,
        CompositeMode::Lighten => BlendMode::Lighten,
        CompositeMode::ColorDodge => BlendMode::ColorDodge,
        CompositeMode::ColorBurn => BlendMode::ColorBurn,
        CompositeMode::HardLight => BlendMode::HardLight,
        CompositeMode::SoftLight => BlendMode::SoftLight,
        CompositeMode::Difference => BlendMode::Difference,
        CompositeMode::Exclusion => BlendMode::Exclusion,
        CompositeMode::Multiply => BlendMode::Multiply,
        CompositeMode::HslHue => BlendMode::HslHue,
        CompositeMode::HslSaturation => BlendMode::HslSaturation,
        CompositeMode::HslColor => BlendMode::HslColor,
        CompositeMode::HslLuminosity => BlendMode::HslLuminosity,
    }
}
