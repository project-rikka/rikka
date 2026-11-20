use paint_document::Figure;
use rikka_app::prelude::*;
use rikka_figure::{Form, Look, ShapeKind};

const MAX_SHOWN: usize = 24;

pub(super) fn describe(figure: &Figure) -> String {
    let id = figure.id.0;
    match &figure.form {
        Form::Line { from, to, pen } | Form::Arrow { from, to, pen } => format!(
            "{id} {} from {} to {}, {}, width {}",
            kind(&figure.form),
            point(*from),
            point(*to),
            pen.color.to_hex(),
            number(pen.width)
        ),
        Form::Shape { frame, look, .. } => {
            let corner = Point::new(
                frame.center.x - frame.width / 2.0,
                frame.center.y - frame.height / 2.0,
            );
            let angle = match frame.angle.abs() > 0.01 {
                true => format!(", angle {}", number(frame.angle)),
                false => String::new(),
            };
            format!(
                "{id} {} rect [{}, {}, {}, {}]{angle}, {}",
                kind(&figure.form),
                number(corner.x),
                number(corner.y),
                number(frame.width),
                number(frame.height),
                looked(look)
            )
        }
        Form::Path(path) => {
            let fill = path
                .fill()
                .map(|(color, _)| format!(", fill {}", color.to_hex()))
                .unwrap_or_default();
            let stroke = path
                .stroke()
                .map(|(color, stroke)| {
                    format!(", stroke {} width {}", color.to_hex(), number(stroke.width))
                })
                .unwrap_or_default();
            format!(
                "{id} path within {}{fill}{stroke}",
                area(figure.form.extent())
            )
        }
        Form::Text(lettering) => {
            let excerpt: String = lettering.text.chars().take(MAX_SHOWN).collect();
            format!(
                "{id} text \"{excerpt}\" within {}, {} {} px, {}",
                area(figure.form.bounds()),
                lettering.font,
                number(lettering.size),
                lettering.color.to_hex()
            )
        }
    }
}

fn kind(form: &Form) -> &'static str {
    match form {
        Form::Line { .. } => "line",
        Form::Arrow { .. } => "arrow",
        Form::Shape { shape, .. } => match shape.kind() {
            ShapeKind::Rectangle => "rectangle",
            ShapeKind::RoundedRectangle => "rounded rectangle",
            ShapeKind::Ellipse => "ellipse",
            ShapeKind::Triangle => "triangle",
            ShapeKind::Polygon => "polygon",
            ShapeKind::Star => "star",
        },
        Form::Path(_) => "path",
        Form::Text(_) => "text",
    }
}

fn looked(look: &Look) -> String {
    match (look.outline, look.fill) {
        (Some(pen), None) => {
            format!("outline {} width {}", pen.color.to_hex(), number(pen.width))
        }
        (None, Some(fill)) => format!("fill {}", fill.to_hex()),
        (Some(pen), Some(fill)) => format!(
            "outline {} width {}, fill {}",
            pen.color.to_hex(),
            number(pen.width),
            fill.to_hex()
        ),
        (None, None) => "no fill or outline".into(),
    }
}

fn point(at: Point) -> String {
    format!("[{}, {}]", number(at.x), number(at.y))
}

fn area(rect: Rect) -> String {
    format!(
        "[{}, {}, {}, {}]",
        number(rect.x()),
        number(rect.y()),
        number(rect.width()),
        number(rect.height())
    )
}

fn number(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    match rounded.fract() == 0.0 {
        true => format!("{rounded:.0}"),
        false => format!("{rounded:.1}"),
    }
}
