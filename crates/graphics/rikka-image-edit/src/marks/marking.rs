use crate::marks::brush::BrushStroke;
use crate::marks::mark::Mark;
use crate::marks::path_mark::PathMark;

#[derive(Clone, Debug, PartialEq)]
pub enum Marking {
    Mark(Mark),
    Brush(BrushStroke),
    Path(PathMark),
}

impl Marking {
    #[must_use]
    pub(crate) fn scaled(&self, factor: f32) -> Marking {
        match self {
            Marking::Mark(mark) => Marking::Mark(mark.scaled(factor)),
            Marking::Brush(stroke) => Marking::Brush(stroke.scaled(factor)),
            Marking::Path(path_mark) => Marking::Path(path_mark.scaled(factor)),
        }
    }
}

impl From<Mark> for Marking {
    fn from(mark: Mark) -> Marking {
        Marking::Mark(mark)
    }
}

impl From<BrushStroke> for Marking {
    fn from(stroke: BrushStroke) -> Marking {
        Marking::Brush(stroke)
    }
}

impl From<PathMark> for Marking {
    fn from(path_mark: PathMark) -> Marking {
        Marking::Path(path_mark)
    }
}
