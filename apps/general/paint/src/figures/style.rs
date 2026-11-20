use paint_document::FigureKind;
use rikka_app::prelude::Color;
use rikka_figure::{Form, Lettering, Look, Pen};
use rikka_image_edit::{Cap, FillRule, Join, PathMark, ShapeStyle, StrokeStyle};

use crate::colors::{Colors, Slot};
use crate::options::{Filling, Letters, THINNEST};
use crate::tool::Tool;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Restyle {
    Colors(Colors),
    Width(f32),
    Filling {
        filling: Filling,
        colors: Colors,
        width: Option<f32>,
    },
    Antialiased(bool),
    Letters(Letters),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StyleSource {
    pub(crate) tool: Tool,
    pub(crate) primary: Color,
    pub(crate) secondary: Option<Color>,
    pub(crate) width: Option<f32>,
    pub(crate) filling: Option<Filling>,
    pub(crate) antialiased: Option<bool>,
    pub(crate) letters: Option<Letters>,
}

pub(crate) fn restyled(form: &Form, restyle: &Restyle) -> Form {
    match form {
        Form::Line { from, to, pen } => Form::Line {
            from: *from,
            to: *to,
            pen: penned(*pen, restyle),
        },
        Form::Arrow { from, to, pen } => Form::Arrow {
            from: *from,
            to: *to,
            pen: penned(*pen, restyle),
        },
        Form::Shape { shape, frame, look } => Form::Shape {
            shape: *shape,
            frame: *frame,
            look: looked(*look, restyle),
        },
        Form::Path(figure) => Form::Path(pathed(figure, restyle)),
        Form::Text(lettering) => Form::Text(lettered(lettering, restyle)),
    }
}

fn penned(pen: Pen, restyle: &Restyle) -> Pen {
    match restyle {
        Restyle::Colors(colors) => Pen {
            color: colors.primary,
            ..pen
        },
        Restyle::Width(width) => Pen {
            width: *width,
            ..pen
        },
        Restyle::Filling { .. } | Restyle::Antialiased(_) | Restyle::Letters(_) => pen,
    }
}

fn looked(look: Look, restyle: &Restyle) -> Look {
    match restyle {
        Restyle::Colors(colors) => match look.outline {
            Some(pen) => Look {
                fill: look.fill.map(|_| colors.secondary),
                outline: Some(Pen {
                    color: colors.primary,
                    ..pen
                }),
            },
            None => Look {
                fill: look.fill.map(|_| colors.primary),
                outline: None,
            },
        },
        Restyle::Width(width) => Look {
            outline: look.outline.map(|pen| Pen {
                width: *width,
                ..pen
            }),
            ..look
        },
        Restyle::Filling {
            filling,
            colors,
            width,
        } => {
            let width = look
                .outline
                .map(|pen| pen.width)
                .or(*width)
                .unwrap_or(THINNEST);
            Look::styled(colors.primary, width, filling.style(*colors, Slot::Primary))
        }
        Restyle::Antialiased(_) | Restyle::Letters(_) => look,
    }
}

fn pathed(figure: &PathMark, restyle: &Restyle) -> PathMark {
    let stroke = figure.stroke();
    let fill = figure.fill();
    let width = stroke.map_or(1.0, |(_, style)| style.width);
    let outline = stroke.map_or_else(
        || StrokeStyle::new(width).join(Join::Round).cap(Cap::Round),
        |(_, style)| style,
    );
    let rule = fill.map_or(FillRule::NonZero, |(_, rule)| rule);
    let (stroke, fill, antialiased) = match restyle {
        Restyle::Colors(colors) => match (stroke, fill) {
            (Some((_, style)), Some((_, rule))) => (
                Some((colors.primary, style)),
                Some((colors.secondary, rule)),
                figure.is_antialiased(),
            ),
            (Some((_, style)), None) => {
                (Some((colors.primary, style)), None, figure.is_antialiased())
            }
            (None, fill) => (
                None,
                fill.map(|(_, rule)| (colors.primary, rule)),
                figure.is_antialiased(),
            ),
        },
        Restyle::Width(width) => (
            stroke.map(|(color, style)| {
                (
                    color,
                    StrokeStyle {
                        width: *width,
                        ..style
                    },
                )
            }),
            fill,
            figure.is_antialiased(),
        ),
        Restyle::Filling {
            filling, colors, ..
        } => {
            let outlined = Some((colors.primary, outline));
            match filling {
                Filling::Outline => (outlined, None, figure.is_antialiased()),
                Filling::Fill => (None, Some((colors.primary, rule)), figure.is_antialiased()),
                Filling::FillAndOutline => (
                    outlined,
                    Some((colors.secondary, rule)),
                    figure.is_antialiased(),
                ),
            }
        }
        Restyle::Antialiased(antialiased) => (stroke, fill, *antialiased),
        Restyle::Letters(_) => (stroke, fill, figure.is_antialiased()),
    };
    let mut restyled = PathMark::new(figure.path().clone()).antialiased(antialiased);
    if let Some((color, rule)) = fill {
        restyled = restyled.filled(color, rule);
    }
    if let Some((color, style)) = stroke {
        restyled = restyled.stroked(color, style);
    }
    restyled
}

fn lettered(lettering: &Lettering, restyle: &Restyle) -> Lettering {
    match restyle {
        Restyle::Colors(colors) => Lettering {
            color: colors.primary,
            ..lettering.clone()
        },
        Restyle::Letters(letters) => {
            letters.lettering(lettering.text.clone(), lettering.origin, lettering.color)
        }
        Restyle::Width(_) | Restyle::Filling { .. } | Restyle::Antialiased(_) => lettering.clone(),
    }
}

pub(crate) fn adopted(form: &Form) -> StyleSource {
    let tool = Tool::drawing_figure(FigureKind::from_form_kind(form.kind()));
    match form {
        Form::Line { pen, .. } | Form::Arrow { pen, .. } => StyleSource {
            tool,
            primary: pen.color,
            secondary: None,
            width: Some(pen.width),
            filling: None,
            antialiased: None,
            letters: None,
        },
        Form::Shape { look, .. } => {
            let (filling, secondary) = match look.style() {
                ShapeStyle::Outline => (Filling::Outline, None),
                ShapeStyle::Fill => (Filling::Fill, None),
                ShapeStyle::FillAndOutline { fill } => (Filling::FillAndOutline, Some(fill)),
            };
            StyleSource {
                tool,
                primary: look.ink(),
                secondary,
                width: look.outline.map(|pen| pen.width),
                filling: Some(filling),
                antialiased: None,
                letters: None,
            }
        }
        Form::Path(figure) => {
            let (filling, primary, secondary) = match (figure.stroke(), figure.fill()) {
                (Some((stroke, _)), Some((fill, _))) => {
                    (Filling::FillAndOutline, stroke, Some(fill))
                }
                (Some((stroke, _)), None) => (Filling::Outline, stroke, None),
                (None, Some((fill, _))) => (Filling::Fill, fill, None),
                (None, None) => (Filling::Outline, Color::BLACK, None),
            };
            StyleSource {
                tool,
                primary,
                secondary,
                width: figure.stroke().map(|(_, style)| style.width),
                filling: Some(filling),
                antialiased: Some(figure.is_antialiased()),
                letters: None,
            }
        }
        Form::Text(lettering) => StyleSource {
            tool,
            primary: lettering.color,
            secondary: None,
            width: None,
            filling: None,
            antialiased: None,
            letters: Some(Letters::from_lettering(lettering)),
        },
    }
}
