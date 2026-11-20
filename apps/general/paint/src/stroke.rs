use paint_document::FigureKind;
use rikka_app::prelude::{Color, Point};
use rikka_figure::{Form, Keys, Look, Pen, constrained_span};
use rikka_image_edit::{Brush, BrushStroke, Ink, Mark, Marking, Path, ShapeStyle, Tool};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Measure {
    Length { pixels: f32, degrees: f32 },
    Size { width: f32, height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum StrokeStyle {
    Mark {
        tool: Tool,
        ink: Color,
        width: f32,
    },
    Shape {
        kind: FigureKind,
        ink: Color,
        width: f32,
        style: ShapeStyle,
    },
    Brush {
        brush: Brush,
        ink: Ink,
    },
    Freeform {
        ink: Color,
        width: f32,
        style: ShapeStyle,
        antialiased: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stroke {
    nib: StrokeStyle,
    trail: Vec<Point>,
    reach: Point,
    held_keys: Keys,
}

impl Stroke {
    pub(crate) fn new(tool: Tool, ink: Color, width: f32, at: Point) -> Stroke {
        Stroke::with(StrokeStyle::Mark { tool, ink, width }, at)
    }

    pub(crate) fn shape(kind: FigureKind, ink: Color, width: f32, at: Point) -> Stroke {
        Stroke::with(
            StrokeStyle::Shape {
                kind,
                ink,
                width,
                style: ShapeStyle::Outline,
            },
            at,
        )
    }

    pub(crate) fn brush(brush: Brush, ink: Ink, at: Point) -> Stroke {
        Stroke::with(StrokeStyle::Brush { brush, ink }, at)
    }

    pub(crate) fn freeform(ink: Color, width: f32, antialiased: bool, at: Point) -> Stroke {
        Stroke::with(
            StrokeStyle::Freeform {
                ink,
                width,
                style: ShapeStyle::Outline,
                antialiased,
            },
            at,
        )
    }

    fn with(nib: StrokeStyle, at: Point) -> Stroke {
        Stroke {
            nib,
            trail: vec![at],
            reach: at,
            held_keys: Keys::default(),
        }
    }

    pub(crate) fn styled(mut self, shaped: ShapeStyle) -> Stroke {
        match &mut self.nib {
            StrokeStyle::Shape { style, .. } | StrokeStyle::Freeform { style, .. } => {
                *style = shaped
            }
            StrokeStyle::Mark { .. } | StrokeStyle::Brush { .. } => {}
        }
        self
    }

    pub(crate) fn keys(mut self, keys: Keys) -> Stroke {
        self.held_keys = keys;
        self
    }

    fn kind(&self) -> Option<FigureKind> {
        match self.nib {
            StrokeStyle::Shape { kind, .. } => Some(kind),
            StrokeStyle::Mark { .. } | StrokeStyle::Brush { .. } | StrokeStyle::Freeform { .. } => {
                None
            }
        }
    }

    fn is_freehand(&self) -> bool {
        self.kind().is_none()
    }

    pub(crate) fn reach(&mut self, at: Point) -> bool {
        if at == self.reach {
            return false;
        }
        self.reach = at;
        if self.is_freehand() {
            self.trail.push(at);
        }
        true
    }

    pub(crate) fn hold(&mut self, keys: Keys) -> bool {
        let changed = self.held_keys != keys;
        self.held_keys = keys;
        changed && !self.is_freehand()
    }

    pub(crate) fn ends(&self) -> (Point, Point) {
        let anchor = self.trail[0];
        match self.kind() {
            Some(kind) => constrained_span(kind.form_kind(), anchor, self.reach, self.held_keys),
            None => (anchor, self.reach),
        }
    }

    pub(crate) fn floats(&self) -> bool {
        matches!(
            self.nib,
            StrokeStyle::Shape { .. } | StrokeStyle::Freeform { .. }
        )
    }

    pub(crate) fn form(&self) -> Option<Form> {
        match self.nib {
            StrokeStyle::Shape {
                kind,
                ink,
                width,
                style,
            } => {
                let (from, to) = self.ends();
                kind.spanned(from, to, Pen::new(ink, width), style)
            }
            StrokeStyle::Freeform {
                ink,
                width,
                style,
                antialiased,
            } => Some(Form::Path(
                Look::styled(ink, width, style)
                    .mark(Path::polygon(self.trail.iter().copied()))
                    .antialiased(antialiased),
            )),
            StrokeStyle::Mark { .. } | StrokeStyle::Brush { .. } => None,
        }
    }

    pub(crate) fn mark(&self, offset: Point) -> Option<Marking> {
        let moved = |point: &Point| Point::new(point.x + offset.x, point.y + offset.y);
        match self.nib {
            StrokeStyle::Mark { tool, ink, width } => {
                let mut mark = Mark::new(
                    tool,
                    ink,
                    width / tool.thickness(1.0),
                    moved(&self.trail[0]),
                );
                for point in &self.trail[1..] {
                    mark.extend_to(moved(point));
                }
                Some(mark.into())
            }
            StrokeStyle::Brush { brush, ink } => {
                BrushStroke::from_points(brush, ink, self.trail.iter().map(moved)).map(Into::into)
            }
            StrokeStyle::Shape { .. } | StrokeStyle::Freeform { .. } => {
                self.form()?.moved(offset).marking()
            }
        }
    }

    pub(crate) fn is_worth_keeping(&self) -> bool {
        match self.nib {
            StrokeStyle::Mark { .. } => match self.mark(Point::ZERO) {
                Some(Marking::Mark(mark)) => mark.is_significant(),
                _ => false,
            },
            StrokeStyle::Brush { .. } => true,
            StrokeStyle::Shape { .. } | StrokeStyle::Freeform { .. } => {
                self.form().is_some_and(|form| form.is_worth_keeping())
            }
        }
    }

    pub(crate) fn measure(&self) -> Option<Measure> {
        let (from, to) = self.ends();
        let (across, down) = (to.x - from.x, to.y - from.y);
        match self.kind()? {
            FigureKind::Line | FigureKind::Arrow => Some(Measure::Length {
                pixels: across.hypot(down),
                degrees: (-down).atan2(across).to_degrees(),
            }),
            FigureKind::Rectangle | FigureKind::Ellipse => Some(Measure::Size {
                width: across.abs(),
                height: down.abs(),
            }),
            FigureKind::Path | FigureKind::Text => None,
        }
    }
}
