use paint_document::{Figure, FigureChange, FigureKind};
use rikka_app::prelude::Point;
use rikka_figure::{Change, Form, GrabTarget, Grip, Grips, Keys, ManipulationResult};

#[derive(Clone, Debug)]
pub(crate) struct Manipulation {
    inner: rikka_figure::Manipulation,
    originals: Vec<Figure>,
}

#[derive(Clone, Debug)]
pub(crate) struct ManipulatedFigures {
    pub(crate) figures: Vec<Figure>,
    pub(crate) grips: Grips,
}

impl Manipulation {
    pub(crate) fn new(grabbed: GrabTarget, from: Point, originals: Vec<Figure>) -> Option<Self> {
        let grips = grips_of(&originals)?;
        Some(Manipulation {
            inner: rikka_figure::Manipulation::new(grabbed, from, grips),
            originals,
        })
    }

    pub(crate) fn originals(&self) -> &[Figure] {
        &self.originals
    }

    pub(crate) fn changing(&self) -> FigureChange {
        match self.inner.change() {
            Change::Move => FigureChange::Move,
            Change::Resize => FigureChange::Resize,
            Change::Rotate => FigureChange::Rotate,
        }
    }

    pub(crate) fn reach(&self, at: Point, keys: Keys) -> ManipulatedFigures {
        match self.inner.drag_to(at, keys) {
            ManipulationResult::Mapped { map, grips } => {
                let sized = matches!(self.inner.target(), GrabTarget::Grip(Grip::Side(..)));
                let scales_text = sized && self.originals.len() == 1;
                let figures = self
                    .originals
                    .iter()
                    .map(|figure| {
                        let text = figure.kind() == FigureKind::Text;
                        figure.with(figure.form.mapped(map, text && scales_text))
                    })
                    .collect();
                ManipulatedFigures { figures, grips }
            }
            ManipulationResult::Ended { from, to, .. } => self.ended(from, to),
        }
    }

    fn ended(&self, from: Point, to: Point) -> ManipulatedFigures {
        let unchanged = || ManipulatedFigures {
            figures: self.originals.clone(),
            grips: self.inner.grips(),
        };
        let [figure] = self.originals.as_slice() else {
            return unchanged();
        };
        let form = match &figure.form {
            Form::Line { pen, .. } => Form::Line {
                from,
                to,
                pen: *pen,
            },
            Form::Arrow { pen, .. } => Form::Arrow {
                from,
                to,
                pen: *pen,
            },
            _ => return unchanged(),
        };
        ManipulatedFigures {
            figures: vec![figure.with(form)],
            grips: Grips::Ends(from, to),
        }
    }
}

pub(crate) fn grips_of(figures: &[Figure]) -> Option<Grips> {
    let forms: Vec<&Form> = figures.iter().map(|figure| &figure.form).collect();
    Grips::for_forms(&forms)
}
