mod bar;

use paint_document::{Combine, Flood, FloodMode, LayerId, Source};
use rikka_app::prelude::Color;
use rikka_figure::{Lettering, TextAlign};
use rikka_image_edit::{Brush, GradientShape, ShapeStyle};

pub(crate) use bar::bar;

use crate::colors::{Colors, Slot};
use crate::tool::Tool;

pub(crate) const THINNEST: f32 = 1.0;
pub(crate) const THICKEST: f32 = 500.0;

const SIZED: [(Tool, f32); 9] = [
    (Tool::Pen, 4.0),
    (Tool::Highlighter, 16.0),
    (Tool::Line, 4.0),
    (Tool::Arrow, 4.0),
    (Tool::Rectangle, 4.0),
    (Tool::Ellipse, 4.0),
    (Tool::Paintbrush, 20.0),
    (Tool::Eraser, 20.0),
    (Tool::FreeformShape, 4.0),
];

pub(crate) const SELECTING: [Combine; 5] = [
    Combine::Replace,
    Combine::Union,
    Combine::Exclude,
    Combine::Intersect,
    Combine::Xor,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SampleSize {
    #[default]
    Pixel,
    Three,
    Five,
}

impl SampleSize {
    pub(crate) const ALL: [SampleSize; 3] =
        [SampleSize::Pixel, SampleSize::Three, SampleSize::Five];

    pub(crate) fn pixels(self) -> u32 {
        match self {
            SampleSize::Pixel => 1,
            SampleSize::Three => 3,
            SampleSize::Five => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SampleFrom {
    #[default]
    Image,
    Layer,
}

impl SampleFrom {
    pub(crate) const ALL: [SampleFrom; 2] = [SampleFrom::Image, SampleFrom::Layer];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sampling {
    pub(crate) size: SampleSize,
    pub(crate) from: SampleFrom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flooding {
    pub(crate) tolerance: f32,
    pub(crate) mode: FloodMode,
    pub(crate) from: SampleFrom,
    pub(crate) anti_aliased: bool,
}

impl Default for Flooding {
    fn default() -> Flooding {
        Flooding {
            tolerance: 0.5,
            mode: FloodMode::Contiguous,
            from: SampleFrom::Layer,
            anti_aliased: true,
        }
    }
}

impl Flooding {
    pub(crate) fn flood(self, (x, y): (f32, f32), layer: Option<LayerId>) -> Option<Flood> {
        let source = match self.from {
            SampleFrom::Image => Source::Image,
            SampleFrom::Layer => Source::Layer(layer?),
        };
        Some(
            Flood::from_point(x, y)
                .tolerance(self.tolerance)
                .mode(self.mode)
                .source(source)
                .anti_aliased(self.anti_aliased),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Filling {
    Outline,
    #[default]
    Fill,
    FillAndOutline,
}

impl Filling {
    pub(crate) const ALL: [Filling; 3] = [Filling::Outline, Filling::Fill, Filling::FillAndOutline];

    pub(crate) fn outlines(self) -> bool {
        self != Filling::Fill
    }

    pub(crate) fn style(self, colors: Colors, slot: Slot) -> ShapeStyle {
        match self {
            Filling::Outline => ShapeStyle::Outline,
            Filling::Fill => ShapeStyle::Fill,
            Filling::FillAndOutline => ShapeStyle::FillAndOutline {
                fill: colors.get(slot.other()),
            },
        }
    }
}

pub(crate) const FIRST_BOX: u32 = 400;
pub(crate) const BOX_WIDEST: f32 = 30_000.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Letters {
    pub(crate) font: String,
    pub(crate) size: f32,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) align: TextAlign,
    pub(crate) antialias: bool,
    pub(crate) wraps: bool,
    pub(crate) box_width: u32,
}

impl Default for Letters {
    fn default() -> Letters {
        let lettering = Lettering::default();
        Letters {
            font: lettering.font,
            size: lettering.size,
            bold: false,
            italic: false,
            align: lettering.align,
            antialias: lettering.antialias,
            wraps: false,
            box_width: FIRST_BOX,
        }
    }
}

impl Letters {
    pub(crate) fn from_lettering(lettering: &Lettering) -> Letters {
        Letters {
            font: lettering.font.clone(),
            size: lettering.size,
            bold: lettering.weight >= Lettering::BOLD,
            italic: lettering.italic,
            align: lettering.align,
            antialias: lettering.antialias,
            wraps: lettering.wrap_width.is_some(),
            box_width: lettering.wrap_width.unwrap_or(FIRST_BOX),
        }
    }

    pub(crate) fn lettering(&self, text: String, origin: (i32, i32), color: Color) -> Lettering {
        Lettering {
            text,
            font: self.font.clone(),
            size: self.size,
            weight: match self.bold {
                true => Lettering::BOLD,
                false => Lettering::REGULAR,
            },
            italic: self.italic,
            align: self.align,
            wrap_width: self.wraps.then_some(self.box_width),
            antialias: self.antialias,
            color,
            origin,
            ..Lettering::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Brushing {
    pub(crate) hardness: f32,
    pub(crate) opacity: f32,
    pub(crate) flow: f32,
}

impl Brushing {
    const PAINTBRUSH: Brushing = Brushing {
        hardness: 0.5,
        opacity: 1.0,
        flow: 1.0,
    };

    const ERASER: Brushing = Brushing {
        hardness: 0.75,
        opacity: 1.0,
        flow: 1.0,
    };

    pub(crate) fn brush(self, size: f32, antialiased: bool) -> Brush {
        Brush {
            hardness: self.hardness,
            opacity: self.opacity,
            flow: self.flow,
            antialiased,
            ..Brush::new(size)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum GradientMode {
    #[default]
    Color,
    Transparency,
}

impl GradientMode {
    pub(crate) const ALL: [GradientMode; 2] = [GradientMode::Color, GradientMode::Transparency];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Gradienting {
    pub(crate) shape: GradientShape,
    pub(crate) mode: GradientMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Setting {
    Hardness,
    Opacity,
    Flow,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Options {
    widths: [f32; SIZED.len()],
    pub(crate) filling: Filling,
    pub(crate) selecting: Combine,
    pub(crate) sampling: Sampling,
    pub(crate) smoothing: bool,
    pub(crate) wand: Flooding,
    pub(crate) bucket: Flooding,
    pub(crate) letters: Letters,
    pub(crate) paintbrush: Brushing,
    pub(crate) eraser: Brushing,
    pub(crate) antialiased: bool,
    pub(crate) gradient: Gradienting,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            widths: SIZED.map(|(_, width)| width),
            filling: Filling::default(),
            selecting: Combine::Replace,
            sampling: Sampling::default(),
            smoothing: true,
            wand: Flooding::default(),
            bucket: Flooding::default(),
            letters: Letters::default(),
            paintbrush: Brushing::PAINTBRUSH,
            eraser: Brushing::ERASER,
            antialiased: true,
            gradient: Gradienting::default(),
        }
    }
}

impl Options {
    pub(crate) fn width(&self, tool: Tool) -> Option<f32> {
        index(tool).map(|at| self.widths[at])
    }

    pub(crate) fn set_width(&mut self, tool: Tool, width: f32) {
        if let Some(at) = index(tool) {
            self.widths[at] = width.clamp(THINNEST, THICKEST);
        }
    }

    pub(crate) fn step_width(&mut self, tool: Tool, grow: bool) {
        if let Some(width) = self.width(tool) {
            self.set_width(tool, stepped(width, grow));
        }
    }

    pub(crate) fn brushing(&self, tool: Tool) -> Option<Brushing> {
        match tool {
            Tool::Paintbrush => Some(self.paintbrush),
            Tool::Eraser => Some(self.eraser),
            _ => None,
        }
    }

    pub(crate) fn set_brushing(&mut self, tool: Tool, setting: Setting, value: f32) {
        let brushing = match tool {
            Tool::Paintbrush => &mut self.paintbrush,
            Tool::Eraser => &mut self.eraser,
            _ => return,
        };
        let value = value.clamp(0.0, 1.0);
        match setting {
            Setting::Hardness => brushing.hardness = value,
            Setting::Opacity => brushing.opacity = value,
            Setting::Flow => brushing.flow = value,
        }
    }
}

fn index(tool: Tool) -> Option<usize> {
    SIZED.iter().position(|(sized, _)| *sized == tool)
}

const BRACKETS: [(f32, f32); 4] = [(10.0, 1.0), (50.0, 5.0), (100.0, 10.0), (200.0, 25.0)];
const LAST_STEP: f32 = 50.0;
fn stepped(width: f32, grow: bool) -> f32 {
    let probe = match grow {
        true => width,
        false => width - 0.001,
    };
    let step = BRACKETS
        .iter()
        .find(|(below, _)| probe < *below)
        .map_or(LAST_STEP, |(_, step)| *step);
    let next = match grow {
        true => (width / step).floor() * step + step,
        false => (width / step).ceil() * step - step,
    };
    next.clamp(THINNEST, THICKEST)
}
