use rikka_image_edit::{Mark, Marking, Tool};

use super::Form;

impl Form {
    #[must_use]
    pub fn marking(&self) -> Option<Marking> {
        Some(match self {
            Form::Line { from, to, pen } => {
                let mut mark = Mark::new(Tool::Line, pen.color, pen.width, *from);
                mark.extend_to(*to);
                mark.into()
            }
            Form::Arrow { from, to, pen } => {
                let mut mark = Mark::new(Tool::Arrow, pen.color, pen.width, *from);
                mark.extend_to(*to);
                mark.into()
            }
            Form::Shape { shape, frame, look } => look.mark(shape.path(frame)).into(),
            Form::Path(figure) => figure.clone().into(),
            Form::Text(_) => return None,
        })
    }
}
