use rikka_figure::{Affine, Form, Frame, Grips, Lettering, Look, Pen, Shape, Typeset};
use rikka_types::{Color, Point, Rect, Size};

use crate::animation::Value;
use crate::placement::{Placement, finite, finite_point, side};
use crate::project::Format;

const MIN_SCALE: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArtField {
    Width,
    Height,
    Corner,
    Fill,
    Stroke,
    StrokeWidth,
    End,
    Size,
    Color,
    Spacing,
    Reveal,
}

impl ArtField {
    pub const ALL: [ArtField; 11] = [
        ArtField::Width,
        ArtField::Height,
        ArtField::Corner,
        ArtField::Fill,
        ArtField::Stroke,
        ArtField::StrokeWidth,
        ArtField::End,
        ArtField::Size,
        ArtField::Color,
        ArtField::Spacing,
        ArtField::Reveal,
    ];

    pub(crate) fn is_length(self) -> bool {
        matches!(
            self,
            ArtField::Width
                | ArtField::Height
                | ArtField::Corner
                | ArtField::StrokeWidth
                | ArtField::End
                | ArtField::Size
                | ArtField::Spacing
        )
    }

    pub(crate) fn clamp_value(self, value: Value) -> Value {
        match (self, value) {
            (ArtField::Width | ArtField::Height | ArtField::StrokeWidth, Value::Scalar(length)) => {
                Value::Scalar(side(length))
            }
            (ArtField::Corner, Value::Scalar(radius)) => Value::Scalar(finite(radius).max(0.0)),
            (ArtField::Size, Value::Scalar(size)) => Value::Scalar(match size.is_finite() {
                true => size.clamp(Lettering::SMALLEST, Lettering::LARGEST),
                false => Lettering::SMALLEST,
            }),
            (ArtField::Reveal, Value::Scalar(reveal)) => Value::Scalar(match reveal.is_finite() {
                true => reveal.clamp(0.0, 1.0),
                false => 1.0,
            }),
            (_, Value::Scalar(value)) => Value::Scalar(finite(value)),
            (_, Value::Point(point)) => Value::Point(finite_point(point)),
            (_, Value::Color(color)) => Value::Color(painted(color)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Art {
    Shape {
        shape: Shape,
        width: f32,
        height: f32,
        look: Look,
    },
    Line {
        reach: Point,
        pen: Pen,
        arrow: bool,
    },
    Text {
        lettering: Lettering,
        reveal: f32,
    },
}

impl Art {
    #[must_use]
    pub fn text(lettering: Lettering) -> Art {
        Art::Text {
            lettering,
            reveal: 1.0,
        }
    }

    #[must_use]
    pub fn lettering(&self) -> Option<&Lettering> {
        match self {
            Art::Text { lettering, .. } => Some(lettering),
            Art::Shape { .. } | Art::Line { .. } => None,
        }
    }

    #[must_use]
    pub fn fields(&self) -> Vec<ArtField> {
        match self {
            Art::Shape { shape, look, .. } => {
                let mut fields = vec![ArtField::Width, ArtField::Height];
                if matches!(shape, Shape::Rectangle { .. }) {
                    fields.push(ArtField::Corner);
                }
                if look.fill.is_some() {
                    fields.push(ArtField::Fill);
                }
                if look.outline.is_some() {
                    fields.extend([ArtField::Stroke, ArtField::StrokeWidth]);
                }
                fields
            }
            Art::Line { .. } => vec![ArtField::End, ArtField::Stroke, ArtField::StrokeWidth],
            Art::Text { .. } => vec![
                ArtField::Size,
                ArtField::Color,
                ArtField::Spacing,
                ArtField::Reveal,
            ],
        }
    }

    #[must_use]
    pub fn value(&self, field: ArtField) -> Option<Value> {
        match (self, field) {
            (Art::Shape { width, .. }, ArtField::Width) => Some(Value::Scalar(*width)),
            (Art::Shape { height, .. }, ArtField::Height) => Some(Value::Scalar(*height)),
            (
                Art::Shape {
                    shape: Shape::Rectangle { radius },
                    ..
                },
                ArtField::Corner,
            ) => Some(Value::Scalar(*radius)),
            (Art::Shape { look, .. }, ArtField::Fill) => look.fill.map(Value::Color),
            (Art::Shape { look, .. }, ArtField::Stroke) => {
                look.outline.map(|pen| Value::Color(pen.color))
            }
            (Art::Shape { look, .. }, ArtField::StrokeWidth) => {
                look.outline.map(|pen| Value::Scalar(pen.width))
            }
            (Art::Line { reach, .. }, ArtField::End) => Some(Value::Point(*reach)),
            (Art::Line { pen, .. }, ArtField::Stroke) => Some(Value::Color(pen.color)),
            (Art::Line { pen, .. }, ArtField::StrokeWidth) => Some(Value::Scalar(pen.width)),
            (Art::Text { lettering, .. }, ArtField::Size) => Some(Value::Scalar(lettering.size)),
            (Art::Text { lettering, .. }, ArtField::Color) => Some(Value::Color(lettering.color)),
            (Art::Text { lettering, .. }, ArtField::Spacing) => {
                Some(Value::Scalar(lettering.letter_spacing))
            }
            (Art::Text { reveal, .. }, ArtField::Reveal) => Some(Value::Scalar(*reveal)),
            _ => None,
        }
    }

    #[must_use]
    pub fn with(&self, field: ArtField, value: Value) -> Art {
        let mut art = self.clone();
        match (&mut art, field, value) {
            (Art::Shape { width, .. }, ArtField::Width, Value::Scalar(value)) => *width = value,
            (Art::Shape { height, .. }, ArtField::Height, Value::Scalar(value)) => *height = value,
            (
                Art::Shape {
                    shape: Shape::Rectangle { radius },
                    ..
                },
                ArtField::Corner,
                Value::Scalar(value),
            ) => *radius = value,
            (Art::Shape { look, .. }, ArtField::Fill, Value::Color(color)) => {
                if let Some(fill) = &mut look.fill {
                    *fill = color;
                }
            }
            (Art::Shape { look, .. }, ArtField::Stroke, Value::Color(color)) => {
                if let Some(pen) = &mut look.outline {
                    pen.color = color;
                }
            }
            (Art::Shape { look, .. }, ArtField::StrokeWidth, Value::Scalar(width)) => {
                if let Some(pen) = &mut look.outline {
                    pen.width = width;
                }
            }
            (Art::Line { reach, .. }, ArtField::End, Value::Point(point)) => *reach = point,
            (Art::Line { pen, .. }, ArtField::Stroke, Value::Color(color)) => pen.color = color,
            (Art::Line { pen, .. }, ArtField::StrokeWidth, Value::Scalar(width)) => {
                pen.width = width;
            }
            (Art::Text { lettering, .. }, ArtField::Size, Value::Scalar(size)) => {
                lettering.size = size;
                lettering.letter_spacing = lettering.letter_spacing.max(-size / 2.0);
            }
            (Art::Text { lettering, .. }, ArtField::Color, Value::Color(color)) => {
                lettering.color = color;
            }
            (Art::Text { lettering, .. }, ArtField::Spacing, Value::Scalar(spacing)) => {
                lettering.letter_spacing = spacing.max(-lettering.size / 2.0);
            }
            (Art::Text { reveal, .. }, ArtField::Reveal, Value::Scalar(value)) => *reveal = value,
            _ => {}
        }
        art
    }

    #[must_use]
    pub fn scaled(&self, factor: f32) -> Art {
        let pen = |pen: Pen| Pen::new(pen.color, pen.width * factor);
        match self {
            Art::Shape {
                shape,
                width,
                height,
                look,
            } => Art::Shape {
                shape: match *shape {
                    Shape::Rectangle { radius } => Shape::Rectangle {
                        radius: radius * factor,
                    },
                    other => other,
                },
                width: side(width * factor),
                height: side(height * factor),
                look: Look {
                    outline: look.outline.map(pen),
                    ..*look
                },
            },
            Art::Line {
                reach,
                pen: stroke,
                arrow,
            } => Art::Line {
                reach: *reach * factor,
                pen: pen(*stroke),
                arrow: *arrow,
            },
            Art::Text { lettering, reveal } => Art::Text {
                lettering: lettering.scaled(factor),
                reveal: *reveal,
            },
        }
    }

    #[must_use]
    pub fn form(&self) -> Form {
        match self {
            Art::Shape {
                shape,
                width,
                height,
                look,
            } => Form::Shape {
                shape: *shape,
                frame: Frame::new(Point::ZERO, *width, *height),
                look: *look,
            },
            Art::Line { reach, pen, arrow } => {
                let (from, to) = (*reach * -0.5, *reach * 0.5);
                match arrow {
                    true => Form::Arrow {
                        from,
                        to,
                        pen: *pen,
                    },
                    false => Form::Line {
                        from,
                        to,
                        pen: *pen,
                    },
                }
            }
            Art::Text { lettering, .. } => Form::Text(centered(lettering)),
        }
    }

    #[must_use]
    pub fn size(&self) -> Size {
        match self {
            Art::Shape { width, height, .. } => Size::new(*width, *height),
            Art::Line { reach, .. } => Size::new(reach.x.abs(), reach.y.abs()),
            Art::Text { lettering, .. } => {
                let layout = Typeset::new(lettering).layout();
                Size::new(layout.width(), layout.height())
            }
        }
    }

    #[must_use]
    pub fn reach(&self) -> Rect {
        self.form().bounds()
    }

    #[must_use]
    pub fn checked(self) -> Art {
        match self {
            Art::Shape {
                shape,
                width,
                height,
                look,
            } => Art::Shape {
                shape: shape.checked(),
                width: side(width),
                height: side(height),
                look,
            },
            Art::Line { reach, pen, arrow } => Art::Line {
                reach: Point::new(finite(reach.x), finite(reach.y)),
                pen: Pen::new(pen.color, side(pen.width)),
                arrow,
            },
            Art::Text { lettering, reveal } => Art::Text {
                lettering,
                reveal: match reveal.is_finite() {
                    true => reveal.clamp(0.0, 1.0),
                    false => 1.0,
                },
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graphic {
    pub placement: Placement,
    pub art: Art,
}

impl Graphic {
    #[must_use]
    pub fn new(art: Art) -> Graphic {
        Graphic {
            placement: Placement::default(),
            art,
        }
    }

    #[must_use]
    pub fn with_placement(self, placement: Placement) -> Graphic {
        Graphic { placement, ..self }
    }

    #[must_use]
    pub fn checked(self) -> Graphic {
        Graphic {
            placement: self.placement.checked(),
            art: self.art.checked(),
        }
    }

    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self.art, Art::Text { .. })
    }

    #[must_use]
    pub fn frame(&self, format: Format) -> Frame {
        self.placement.frame(self.art.size(), format)
    }

    #[must_use]
    pub fn ends(&self, format: Format) -> Option<(Point, Point)> {
        let Art::Line { reach, .. } = &self.art else {
            return None;
        };
        let map = self.placement.affine(format);
        Some((map.apply(*reach * -0.5), map.apply(*reach * 0.5)))
    }

    #[must_use]
    pub fn grips(&self, format: Format) -> Grips {
        match self.ends(format) {
            Some((from, to)) => Grips::Ends(from, to),
            None => Grips::Frame {
                frame: self.frame(format),
                sides: !self.is_text(),
                turns: true,
            },
        }
    }

    #[must_use]
    pub fn bounds(&self, format: Format) -> Rect {
        let reach = self.art.reach();
        let map = self.placement.affine(format);
        rikka_figure::enclosing(
            [
                Point::new(reach.min_x(), reach.min_y()),
                Point::new(reach.max_x(), reach.min_y()),
                Point::new(reach.max_x(), reach.max_y()),
                Point::new(reach.min_x(), reach.max_y()),
            ]
            .map(|corner| map.apply(corner)),
        )
    }

    #[must_use]
    pub fn hits(&self, format: Format, at: Point, reach: f32) -> bool {
        let Some(inverse) = self.placement.affine(format).inverse() else {
            return false;
        };
        self.art
            .form()
            .hits(inverse.apply(at), reach / self.placement.least_scale())
    }

    #[must_use]
    pub fn mapped(&self, map: Affine, format: Format) -> Graphic {
        if let Some((from, to)) = self.ends(format) {
            return self.with_endpoints(map.apply(from), map.apply(to), format);
        }
        let before = self.frame(format);
        let after = before.mapped(map);
        let (across, down) = self.placement.scale();
        match &self.art {
            Art::Shape { shape, look, .. } => {
                let art = Art::Shape {
                    shape: *shape,
                    width: side(after.width / across.max(MIN_SCALE)),
                    height: side(after.height / down.max(MIN_SCALE)),
                    look: *look,
                };
                Graphic {
                    placement: self.placement.followed(&after, format),
                    art,
                }
            }
            Art::Text { .. } => Graphic {
                placement: self.placement.grown(&before, &after, format),
                art: self.art.clone(),
            },
            Art::Line { .. } => self.clone(),
        }
    }

    #[must_use]
    pub fn with_endpoints(&self, from: Point, to: Point, format: Format) -> Graphic {
        let Art::Line { pen, arrow, .. } = &self.art else {
            return self.clone();
        };
        let center = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
        let (across, down) = self.placement.scale();
        let unturned =
            Affine::rotation(-f64::from(self.placement.rotation)).apply_vector(to - from);
        let reach = Point::new(
            unturned.x / across.max(MIN_SCALE),
            unturned.y / down.max(MIN_SCALE),
        );
        Graphic {
            placement: self.placement.centered_at(center, format),
            art: Art::Line {
                reach,
                pen: *pen,
                arrow: *arrow,
            },
        }
    }

    #[must_use]
    pub fn with_text(&self, text: String) -> Graphic {
        match &self.art {
            Art::Text { lettering, reveal } => Graphic {
                placement: self.placement,
                art: Art::Text {
                    lettering: Lettering {
                        text,
                        ..lettering.clone()
                    },
                    reveal: *reveal,
                },
            },
            Art::Shape { .. } | Art::Line { .. } => self.clone(),
        }
    }

    #[must_use]
    pub fn moved(&self, by: Point) -> Graphic {
        Graphic {
            placement: self.placement.moved(by),
            art: self.art.clone(),
        }
    }
}

fn centered(lettering: &Lettering) -> Lettering {
    let layout = Typeset::new(&Lettering {
        origin: (0, 0),
        ..lettering.clone()
    })
    .layout();
    Lettering {
        origin: (
            -(layout.width() / 2.0).round() as i32,
            -(layout.height() / 2.0).round() as i32,
        ),
        ..lettering.clone()
    }
}

fn painted(color: Color) -> Color {
    let channel = |value: f32| match value.is_finite() {
        true => value.clamp(0.0, 1.0),
        false => 0.0,
    };
    Color::rgba(
        channel(color.r),
        channel(color.g),
        channel(color.b),
        channel(color.a),
    )
}
