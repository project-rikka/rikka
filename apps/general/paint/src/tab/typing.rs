use paint_document::{AddMethod, Edit, Figure, FigureChange, FigureEdit, Layer};
use rikka_app::prelude::*;
use rikka_figure::Form;

use super::Tab;
use crate::options::Letters;
use crate::typing::Typing;

impl Tab {
    pub(crate) fn typing(&self) -> &Typing {
        &self.typing
    }

    pub(crate) fn edit_text(&mut self, figure: &Figure) {
        let Form::Text(lettering) = &figure.form else {
            return;
        };
        let Some(layer) = Layer::holding(&self.layers, figure.id).map(|layer| layer.id) else {
            return;
        };
        if !self.figures_editable() {
            return;
        }
        self.unpick();
        self.active_layer = Some(layer);
        let others = self
            .figures_of(layer)
            .iter()
            .filter(|other| other.id != figure.id)
            .cloned()
            .collect();
        self.lift(layer, others);
        self.typing.edit(figure.id, lettering);
    }

    pub(crate) fn drop_text(&mut self) {
        self.typing.take();
        self.end_lift(None);
    }

    pub(crate) fn commit_text(&mut self, letters: &Letters, color: Color) -> Option<Label> {
        let draft = self.typing.draft()?;
        if let Some(refusal) = self.draw_refusal() {
            return Some(refusal);
        }
        self.typing.take();
        let lettering = draft.lettering(letters, color);
        let blank = lettering.text.trim().is_empty();
        let Some(id) = draft.editing else {
            if !blank {
                self.add_figures(
                    self.active_layer?,
                    vec![Form::Text(lettering.clone())],
                    AddMethod::Draw,
                );
                self.typing.settle(lettering, self.ordered);
            }
            return None;
        };
        let Some(layer) = Layer::holding(&self.layers, id).map(|layer| layer.id) else {
            self.end_lift(None);
            return None;
        };
        let current = self
            .figures_of(layer)
            .iter()
            .find(|figure| figure.id == id)
            .map(|figure| figure.form.clone());
        match (blank, current) {
            (true, _) => {
                self.order(Edit::Figures {
                    layer,
                    change: FigureEdit::Remove(vec![id]),
                });
                self.end_lift(Some(self.ordered));
            }
            (false, Some(Form::Text(current))) if current == lettering => self.end_lift(None),
            (false, _) => {
                self.change_figures(
                    layer,
                    vec![Figure::new(id, Form::Text(lettering.clone()))],
                    FigureChange::Retext,
                );
                self.unpick();
                self.typing.settle(lettering, self.ordered);
            }
        }
        None
    }
}
