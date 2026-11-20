use paint_assistant::ObjectStep;
use paint_document::{
    AddMethod, Edit, Figure, FigureChange, FigureEdit, FigureId, Layer, LayerId, LayerKind,
};
use rikka_figure::Affine;
use rikka_types::Point;

use super::super::layers::every_layer;
use super::{Composer, Refusal, refused, stepped};

impl Composer<'_> {
    pub(super) fn objects(&mut self, steps: Vec<ObjectStep>) -> Result<(), Refusal> {
        for (at, step) in steps.into_iter().enumerate() {
            let kind = kind(&step);
            let done = self.object(step);
            stepped("step", at, kind, done)?;
        }
        Ok(())
    }

    fn object(&mut self, step: ObjectStep) -> Result<(), Refusal> {
        match step {
            ObjectStep::Move { objects, offset } => {
                let (layer, figures) = self.resolve_figures(&objects)?;
                let moved = figures
                    .iter()
                    .map(|figure| figure.with(figure.form.moved(offset)))
                    .collect();
                self.change(layer, moved, FigureChange::Move)
            }
            ObjectStep::Rotate {
                objects,
                degrees,
                around,
            } => {
                let (layer, figures) = self.resolve_figures(&objects)?;
                let center = around.unwrap_or_else(|| {
                    figures
                        .iter()
                        .map(|figure| figure.form.extent())
                        .reduce(|whole, one| whole.union(&one))
                        .map_or(Point::ZERO, |whole| whole.center())
                });
                let map = Affine::about(center, Affine::rotation(f64::from(degrees)));
                let turned = figures
                    .iter()
                    .map(|figure| figure.with(figure.form.mapped(map, false)))
                    .collect();
                self.change(layer, turned, FigureChange::Rotate)
            }
            ObjectStep::Set { object, item } => {
                let (layer, figures) = self.resolve_figures(&[object])?;
                let mut forms = self.forms(item)?;
                let (Some(form), Some(figure)) = (forms.pop(), figures.first()) else {
                    return Err(refused("the item draws nothing"));
                };
                self.change(layer, vec![figure.with(form)], FigureChange::Restyle)
            }
            ObjectStep::Delete { objects } => {
                let (layer, figures) = self.resolve_figures(&objects)?;
                self.apply(&Edit::Figures {
                    layer,
                    change: FigureEdit::Remove(figures.iter().map(|figure| figure.id).collect()),
                })
            }
            ObjectStep::Arrange { objects, stack } => {
                let (layer, figures) = self.resolve_figures(&objects)?;
                self.apply(&Edit::Figures {
                    layer,
                    change: FigureEdit::Stack {
                        figures: figures.iter().map(|figure| figure.id).collect(),
                        to: stack,
                    },
                })
            }
            ObjectStep::Duplicate { objects, offset } => {
                let (layer, figures) = self.resolve_figures(&objects)?;
                let forms = figures
                    .iter()
                    .map(|figure| figure.form.moved(offset))
                    .collect();
                self.place(layer, forms, AddMethod::Duplicate)
            }
            ObjectStep::Rasterize { layer } => {
                let layer = self.layer(layer.as_ref())?;
                if layer.kind != LayerKind::Pixel || layer.figures.is_empty() {
                    return Err(refused(format!(
                        "layer {} holds no objects",
                        layer.id.index()
                    )));
                }
                self.apply(&Edit::Figures {
                    layer: layer.id,
                    change: FigureEdit::Rasterize,
                })
            }
        }
    }

    fn change(
        &mut self,
        layer: LayerId,
        figures: Vec<Figure>,
        by: FigureChange,
    ) -> Result<(), Refusal> {
        let ids: Vec<FigureId> = figures.iter().map(|figure| figure.id).collect();
        self.apply(&Edit::Figures {
            layer,
            change: FigureEdit::Change { figures, by },
        })?;
        self.describe(&ids);
        Ok(())
    }

    fn resolve_figures(&self, objects: &[u64]) -> Result<(LayerId, Vec<Figure>), Refusal> {
        let layers = self.document.layers();
        let mut layer: Option<&Layer> = None;
        for raw in objects {
            let id = FigureId(*raw);
            let Some(holder) = Layer::holding(&layers, id) else {
                return Err(refused(format!(
                    "there is no object {raw}; Document lists the objects: {}",
                    every_id(&layers)
                )));
            };
            match layer {
                Some(other) if other.id != holder.id => {
                    return Err(refused(format!(
                        "the objects of one step share a layer; {raw} is on layer {}, not {}",
                        holder.id.index(),
                        other.id.index()
                    )));
                }
                _ => layer = Some(holder),
            }
        }
        let layer = layer.ok_or_else(|| refused("no objects were named"))?;
        let figures = layer
            .figures
            .iter()
            .filter(|figure| objects.contains(&figure.id.0))
            .cloned()
            .collect();
        Ok((layer.id, figures))
    }
}

fn every_id(layers: &[Layer]) -> String {
    let ids: Vec<String> = every_layer(layers)
        .into_iter()
        .flat_map(|layer| &layer.figures)
        .map(|figure| figure.id.0.to_string())
        .collect();
    match ids.is_empty() {
        true => "none".to_owned(),
        false => ids.join(", "),
    }
}

fn kind(step: &ObjectStep) -> &'static str {
    match step {
        ObjectStep::Move { .. } => "move",
        ObjectStep::Rotate { .. } => "rotate",
        ObjectStep::Set { .. } => "set",
        ObjectStep::Delete { .. } => "delete",
        ObjectStep::Arrange { .. } => "arrange",
        ObjectStep::Duplicate { .. } => "duplicate",
        ObjectStep::Rasterize { .. } => "rasterize",
    }
}
