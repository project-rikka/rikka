use paint_document::{
    AddMethod, Edit, Figure, FigureChange, FigureEdit, FigureId, Layer, LayerId, LayerKind,
    StackOrder,
};
use rikka_app::prelude::*;
use rikka_figure::Form;

use super::Tab;
use crate::figures::{FigureSelection, Restyle, clipboard, restyled};

#[derive(Debug)]
pub(super) struct LiftedFigures {
    layer: LayerId,
    figures: Vec<Figure>,
}

impl Tab {
    pub(crate) fn picked(&self) -> &FigureSelection {
        &self.picked
    }

    pub(crate) fn figures_of(&self, layer: LayerId) -> &[Figure] {
        Layer::find(&self.layers, layer).map_or(&[], |layer| layer.figures.as_slice())
    }

    pub(crate) fn picked_layer(&self) -> Option<LayerId> {
        let first = *self.picked.ids().first()?;
        Layer::holding(&self.layers, first).map(|layer| layer.id)
    }

    pub(crate) fn picked_figures(&self) -> Vec<Figure> {
        let Some(layer) = self.picked_layer() else {
            return Vec::new();
        };
        self.figures_of(layer)
            .iter()
            .filter(|figure| self.picked.holds(figure.id))
            .cloned()
            .collect()
    }

    pub(crate) fn has_picks(&self) -> bool {
        !self.picked.is_empty()
    }

    pub(crate) fn figures_editable(&self) -> bool {
        self.takes_selection()
    }

    pub(crate) fn holds_figures(&self) -> bool {
        self.active_layer()
            .is_some_and(|layer| layer.kind == LayerKind::Pixel && !layer.figures.is_empty())
    }

    pub(crate) fn pick(&mut self, layer: LayerId, ids: Vec<FigureId>) {
        self.commit_restyle();
        self.active_layer = Some(layer);
        self.picked.set(ids);
        self.figure_menu = None;
    }

    pub(crate) fn unpick(&mut self) {
        self.commit_restyle();
        self.picked.set(Vec::new());
        self.figure_menu = None;
    }

    pub(crate) fn commit_restyle(&mut self) {
        if let Some(restyle) = self.restyling.take() {
            self.restyle_picked(&restyle, true);
        }
    }

    pub(crate) fn pick_all(&mut self) {
        let Some(layer) = self.active_layer else {
            return;
        };
        let ids = self
            .figures_of(layer)
            .iter()
            .map(|figure| figure.id)
            .collect();
        self.pick(layer, ids);
    }

    pub(crate) fn figure_menu(&self) -> Option<Point> {
        self.figure_menu
    }

    pub(crate) fn open_figure_menu(&mut self, at: Point) {
        self.figure_menu = Some(at);
    }

    pub(crate) fn close_figure_menu(&mut self) {
        self.figure_menu = None;
    }

    pub(crate) fn add_figures(&mut self, layer: LayerId, forms: Vec<Form>, by: AddMethod) {
        let Some(ids) = self.figure_ids.clone() else {
            return;
        };
        if forms.is_empty() || !self.figures_editable() {
            return;
        }
        let figures: Vec<Figure> = forms
            .into_iter()
            .map(|form| Figure::new(ids.next(), form))
            .collect();
        self.order(Edit::Figures {
            layer,
            change: FigureEdit::Add {
                figures: figures.clone(),
                by,
            },
        });
        self.pick(layer, figures.iter().map(|figure| figure.id).collect());
        self.picked.settle(figures, self.ordered);
        self.end_lift(Some(self.ordered));
    }

    pub(crate) fn change_figures(
        &mut self,
        layer: LayerId,
        figures: Vec<Figure>,
        by: FigureChange,
    ) {
        if figures.is_empty() || !self.figures_editable() {
            self.end_lift(None);
            return;
        }
        self.order(Edit::Figures {
            layer,
            change: FigureEdit::Change {
                figures: figures.clone(),
                by,
            },
        });
        self.picked.settle(figures, self.ordered);
        self.end_lift(Some(self.ordered));
    }

    pub(crate) fn preview_picked(&mut self, live: Vec<Figure>, copies: bool) {
        let Some(layer) = self.picked_layer() else {
            return;
        };
        let committed = self.figures_of(layer);
        let figures = match copies {
            true => committed.iter().chain(&live).cloned().collect(),
            false => committed
                .iter()
                .map(|figure| {
                    live.iter()
                        .find(|moved| moved.id == figure.id)
                        .unwrap_or(figure)
                        .clone()
                })
                .collect(),
        };
        self.picked.show_live(Some(live));
        self.lift(layer, figures);
    }

    pub(crate) fn commit_picked(&mut self, live: Vec<Figure>, by: FigureChange, copies: bool) {
        let Some(layer) = self.picked_layer() else {
            self.drop_picked_preview();
            return;
        };
        match copies {
            true => self.add_figures(
                layer,
                live.into_iter().map(|figure| figure.form).collect(),
                AddMethod::Duplicate,
            ),
            false if live == self.picked_figures() => self.drop_picked_preview(),
            false => self.change_figures(layer, live, by),
        }
    }

    pub(crate) fn drop_picked_preview(&mut self) {
        self.picked.show_live(None);
        self.end_lift(None);
    }

    pub(crate) fn restyle_picked(&mut self, restyle: &Restyle, commit: bool) {
        self.restyling = (!commit).then(|| restyle.clone());
        let picked = self.picked_figures();
        if picked.is_empty() || !self.figures_editable() {
            return;
        }
        let restyled: Vec<Figure> = picked
            .iter()
            .map(|figure| figure.with(restyled(&figure.form, restyle)))
            .collect();
        match commit {
            true => match restyled == picked {
                true => self.drop_picked_preview(),
                false => {
                    let Some(layer) = self.picked_layer() else {
                        return;
                    };
                    self.change_figures(layer, restyled, FigureChange::Restyle);
                }
            },
            false => self.preview_picked(restyled, false),
        }
    }

    pub(crate) fn nudge_picked(&mut self, (dx, dy): (i32, i32)) {
        let Some(layer) = self.picked_layer() else {
            return;
        };
        let by = Point::new(dx as f32, dy as f32);
        let moved = self
            .picked_figures()
            .iter()
            .map(|figure| figure.with(figure.form.moved(by)))
            .collect();
        self.change_figures(layer, moved, FigureChange::Move);
    }

    pub(crate) fn remove_picked(&mut self) {
        let (Some(layer), ids) = (self.picked_layer(), self.picked.ids()) else {
            return;
        };
        if !self.figures_editable() {
            return;
        }
        self.order(Edit::Figures {
            layer,
            change: FigureEdit::Remove(ids),
        });
        self.unpick();
    }

    pub(crate) fn stack_picked(&mut self, to: StackOrder) {
        let (Some(layer), figures) = (self.picked_layer(), self.picked.ids()) else {
            return;
        };
        if self.figures_editable() {
            self.order(Edit::Figures {
                layer,
                change: FigureEdit::Stack { figures, to },
            });
        }
    }

    pub(crate) fn rasterize_figures(&mut self) {
        if let Some(layer) = self.active_layer
            && self.holds_figures()
            && self.figures_editable()
        {
            self.order(Edit::Figures {
                layer,
                change: FigureEdit::Rasterize,
            });
            self.unpick();
        }
    }

    pub(crate) fn copy_picked(&self) {
        clipboard::copy(
            self.picked_figures()
                .into_iter()
                .map(|figure| figure.form)
                .collect(),
        );
    }

    pub(crate) fn duplicate_picked(&mut self) {
        let Some(layer) = self.picked_layer() else {
            return;
        };
        let forms: Vec<Form> = self
            .picked_figures()
            .into_iter()
            .map(|figure| figure.form)
            .collect();
        self.add_figures(layer, clipboard::offset(&forms), AddMethod::Duplicate);
    }

    pub(crate) fn paste_figures(&mut self) {
        let layer = self.picked_layer().or(self.active_layer);
        if let Some(layer) = layer
            && self
                .active_layer()
                .is_some_and(|active_layer| active_layer.kind == LayerKind::Pixel)
        {
            self.add_figures(layer, clipboard::paste(), AddMethod::Paste);
        }
    }

    pub(crate) fn lift(&mut self, layer: LayerId, figures: Vec<Figure>) {
        let scale = self.camera.scale();
        if let Some(worker) = &self.worker {
            self.previews.request_id += 1;
            self.previews.scale = scale;
            worker.preview_figures(self.previews.request_id, layer, figures.clone(), scale);
        }
        self.lift = Some(LiftedFigures { layer, figures });
    }

    pub(crate) fn end_lift(&mut self, retire: Option<u64>) {
        if self.lift.take().is_some() {
            self.end_preview(retire);
        }
    }

    pub(super) fn relift(&mut self) -> bool {
        let Some(LiftedFigures { layer, figures }) = self.lift.take() else {
            return false;
        };
        if self.camera.scale() == self.previews.scale {
            self.lift = Some(LiftedFigures { layer, figures });
            return false;
        }
        self.lift(layer, figures);
        true
    }

    pub(super) fn keep_picks(&mut self) {
        let layers = &self.layers;
        self.picked.keep(|id| Layer::holding(layers, id).is_some());
    }
}
