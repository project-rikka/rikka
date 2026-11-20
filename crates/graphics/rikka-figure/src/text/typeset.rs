use std::ops::Range;
use std::sync::Arc;

use rikka_text::{
    Affinity, Align, Cursor, Family, FontStyle, ParagraphLayout, TextLayout, TextSpec, Weight, Wrap,
};
use rikka_types::{Color, Point, Rect};

use super::lettering::{Lettering, TextAlign};
use super::raster::{self, Raster};

const TAB_WIDTH: u16 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub layout: Rect,
    pub ink: Option<Rect>,
    pub baseline: f32,
    pub line_height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextHit {
    pub at: usize,
    pub affinity: Affinity,
}

#[derive(Clone, Debug)]
pub struct Typeset {
    laid: Arc<TextLayout>,
    origin: (i32, i32),
    width: f32,
    color: Color,
    antialias: bool,
}

impl Typeset {
    #[must_use]
    pub fn new(lettering: &Lettering) -> Typeset {
        let style = match lettering.italic {
            true => FontStyle::Italic,
            false => FontStyle::Normal,
        };
        let spec = TextSpec::new(lettering.text.as_str(), lettering.size)
            .family(Family::parse(&lettering.font))
            .weight(Weight(f32::from(lettering.weight)))
            .style(style)
            .line_height(lettering.line_height())
            .letter_spacing(lettering.letter_spacing)
            .tab_width(TAB_WIDTH);
        let align = match lettering.align {
            TextAlign::Left => Align::Left,
            TextAlign::Center => Align::Center,
            TextAlign::Right => Align::Right,
        };
        let (wrap, width) = match lettering.wrap_width {
            Some(width) => (Wrap::WordOrGlyph, width as f32),
            None => {
                let natural = spec.clone().wrap(Wrap::None).layout(None);
                (Wrap::None, natural.widest().ceil())
            }
        };
        Typeset {
            laid: spec.wrap(wrap).align(align).layout(Some(width)),
            origin: lettering.origin,
            width,
            color: lettering.color,
            antialias: lettering.antialias,
        }
    }

    #[must_use]
    pub fn laid(&self) -> &Arc<TextLayout> {
        &self.laid
    }

    #[must_use]
    pub fn color(&self) -> Color {
        self.color
    }

    #[must_use]
    pub fn origin(&self) -> Point {
        Point::new(self.origin.0 as f32, self.origin.1 as f32)
    }

    #[must_use]
    pub fn layout(&self) -> Rect {
        Rect::new(
            self.origin.0 as f32,
            self.origin.1 as f32,
            self.width,
            self.laid.height(),
        )
    }

    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.laid.line_height()
    }

    #[must_use]
    pub fn measure(&self) -> Measure {
        let baseline = self.laid.rows().next().map_or(0.0, |row| row.baseline());
        Measure {
            layout: self.layout(),
            ink: raster::bounds(&self.laid, self.origin),
            baseline: self.origin.1 as f32 + baseline,
            line_height: self.line_height(),
        }
    }

    #[must_use]
    pub fn raster(&self, within: Rect) -> Option<Raster> {
        raster::raster(&self.laid, self.origin, within, self.color, self.antialias)
    }

    #[must_use]
    pub fn caret(&self, at: usize, affinity: Affinity) -> Rect {
        let Cursor { line, index, .. } = self.laid.cursor_of(at);
        let caret = self.laid.caret(Cursor {
            line,
            index,
            affinity,
        });
        Rect::new(
            self.origin.0 as f32 + caret.x,
            self.origin.1 as f32 + caret.top,
            0.0,
            caret.height,
        )
    }

    #[must_use]
    pub fn paragraph(&self, line: usize) -> Option<&ParagraphLayout> {
        self.laid.paragraph(line)
    }

    #[must_use]
    pub fn hit(&self, point: Point) -> TextHit {
        let cursor = self.laid.hit(
            point.x - self.origin.0 as f32,
            point.y - self.origin.1 as f32,
        );
        TextHit {
            at: self.laid.offset_of(cursor),
            affinity: cursor.affinity,
        }
    }

    #[must_use]
    pub fn selection(&self, range: Range<usize>) -> Vec<Rect> {
        let (x, y) = (self.origin.0 as f32, self.origin.1 as f32);
        self.laid
            .selection_rects(range)
            .into_iter()
            .map(|(_, rect)| rect.translate(Point::new(x, y)))
            .collect()
    }
}
