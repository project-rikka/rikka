use std::rc::Rc;

use rikka_types::Size;
use rikka_ui::{Family, TextRun, font, text_width};

const LEADING: f32 = 1.3;
const REFERENCE: &str = "M";

#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    family: Rc<str>,
    size: f32,
}

impl Font {
    pub(crate) fn new(family: &Rc<str>, size: f32) -> Self {
        Self {
            family: Rc::clone(family),
            size,
        }
    }

    pub(crate) fn line_height(&self) -> f32 {
        (self.size * LEADING).round().max(1.0)
    }

    pub(crate) fn cell(&self, faces: &Faces) -> Size {
        let run = faces.run(false, false, REFERENCE);
        let width = text_width(&run);
        Size::new(width.max(1.0), self.line_height())
    }

    pub(crate) fn faces(&self) -> Faces {
        Faces([
            self.face(false, false),
            self.face(true, false),
            self.face(false, true),
            self.face(true, true),
        ])
    }

    fn face(&self, bold: bool, italic: bool) -> TextRun {
        let face = TextRun::new("", self.size)
            .family(Family::parse(self.family.as_ref()))
            .line_height(self.line_height())
            .no_wrap()
            .weight(if bold { font::BOLD } else { font::REGULAR });
        if italic { face.italic() } else { face }
    }
}

#[derive(Debug)]
pub(crate) struct Faces([TextRun; 4]);

impl Faces {
    pub(crate) fn run(&self, bold: bool, italic: bool, content: &str) -> TextRun {
        self.0[usize::from(bold) | usize::from(italic) << 1]
            .clone()
            .content(content)
    }
}
