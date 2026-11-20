use std::collections::HashSet;
use std::mem;

use rikka_figure::{Affine, Form};
use rikka_image_edit::{Picture, Recipe};
use rikka_psd::{Compression, Info};

use super::{Document, Recorded, no_layer};
use crate::arrange;
use crate::develop;
use crate::edit::{AddMethod, FigureEdit, PixelChange, StackOrder};
use crate::error::{Error, Result};
use crate::figure::{self, Figure, FigureId, FigureIds};
use crate::history::{Action, FigureAction};
use crate::layer::{self, LayerId};
use crate::overlay::{self, Overlay, Overlays, PLAIN};
use crate::state::{Content, Swap, picture_bytes};

pub(super) struct Refit<'a> {
    pub(super) recipe: &'a dyn Fn(bool) -> Recipe,
    pub(super) map: Affine,
    pub(super) strokes: bool,
    pub(super) uprights_text: bool,
}

pub(super) enum FigureOrigin {
    Kept,
    Copied { from: LayerId },
    Merged,
    Unlocked,
}

impl Document {
    #[must_use]
    pub fn figure_ids(&self) -> FigureIds {
        self.figure_ids.clone()
    }

    pub(super) fn key_of(&self, id: LayerId) -> Result<Option<u32>> {
        match &self.state.content {
            Content::Picture { .. } => match id == LayerId(0) {
                true => Ok(Some(PLAIN)),
                false => Err(no_layer(id)),
            },
            Content::Photoshop { document, .. } => Ok(develop::pixel_layer(document, id)?.id()),
        }
    }

    pub(super) fn overlay_of(&self, id: LayerId) -> Result<Option<(u32, Overlay)>> {
        Ok(self
            .key_of(id)?
            .and_then(|key| Some((key, self.state.overlays.get(key)?.clone()))))
    }

    pub(super) fn figure(
        &mut self,
        layer: LayerId,
        change: &FigureEdit,
    ) -> Result<Option<Recorded>> {
        let layer_overlay = self.overlay_of(layer)?;
        let before = layer_overlay
            .as_ref()
            .map_or(&[][..], |(_, overlay)| overlay.figures.as_slice());
        let (after, figured) = self.figured(before, change)?;
        if after == before {
            return Ok(None);
        }
        let action = Action::Figure(figured);
        if let (FigureEdit::Rasterize, Some((key, _))) = (change, &layer_overlay) {
            let mut overlays = self.state.overlays.clone();
            overlays.set(*key, None);
            return Ok(Some(Recorded {
                action,
                swaps: vec![self.state.overlay(overlays, 0)],
                pixels: PixelChange::Unchanged,
            }));
        }
        let (base, linear) = match layer_overlay {
            Some((_, overlay)) => (overlay.base, overlay.linear),
            None => self.drawn_source(layer)?,
        };
        let baked = overlay::baked(&base, &after)?;
        let mut swaps = Vec::new();
        let key = self.write_layer(layer, &baked, linear, &mut swaps)?;
        let mut overlays = self.state.overlays.clone();
        overlays.set(
            key,
            (!after.is_empty()).then_some(Overlay {
                base,
                figures: after,
                linear,
            }),
        );
        swaps.push(self.state.overlay(overlays, 0));
        Ok(Some(Recorded {
            action,
            swaps,
            pixels: PixelChange::Content,
        }))
    }

    fn figured(
        &self,
        before: &[Figure],
        change: &FigureEdit,
    ) -> Result<(Vec<Figure>, FigureAction)> {
        let index_of = |id: FigureId| {
            before
                .iter()
                .position(|figure| figure.id == id)
                .ok_or_else(|| missing(id))
        };
        match change {
            FigureEdit::Add { figures, by } => {
                let first = figures.first().ok_or_else(|| Error::NothingToAdd)?;
                let mut fresh = HashSet::new();
                for figure in figures {
                    if !fresh.insert(figure.id) || self.state.overlays.owner(figure.id).is_some() {
                        return Err(Error::FigureExists(figure.id));
                    }
                    checked(&figure.form)?;
                }
                figures
                    .iter()
                    .for_each(|figure| self.figure_ids.claim(figure.id));
                let figured = match by {
                    AddMethod::Draw => FigureAction::Draw(first.kind()),
                    AddMethod::Paste => FigureAction::Paste,
                    AddMethod::Duplicate => FigureAction::Duplicate,
                };
                Ok(([before, figures].concat(), figured))
            }
            FigureEdit::Change { figures, by } => {
                let mut after = before.to_vec();
                for figure in figures {
                    checked(&figure.form)?;
                    after[index_of(figure.id)?] = figure.clone();
                }
                Ok((after, FigureAction::Change(*by)))
            }
            FigureEdit::Remove(ids) => {
                for id in ids {
                    index_of(*id)?;
                }
                let after = before
                    .iter()
                    .filter(|figure| !ids.contains(&figure.id))
                    .cloned()
                    .collect();
                Ok((after, FigureAction::Delete))
            }
            FigureEdit::Stack { figures, to } => {
                for id in figures {
                    index_of(*id)?;
                }
                Ok((stacked(before, figures, *to), FigureAction::Stack(*to)))
            }
            FigureEdit::Rasterize => Ok((Vec::new(), FigureAction::Rasterize)),
        }
    }

    pub(super) fn drawn_source(&self, layer: LayerId) -> Result<(Picture, bool)> {
        match &self.state.content {
            Content::Picture { picture, .. } => match layer == LayerId(0) {
                true => Ok((picture.clone(), false)),
                false => Err(no_layer(layer)),
            },
            Content::Photoshop { .. } => {
                let placed = self.placed(layer)?;
                let linear = placed.linear();
                Ok((placed.picture, linear))
            }
        }
    }

    pub(super) fn developed_source(&self, layer: LayerId) -> Result<Picture> {
        match self.overlay_of(layer)? {
            Some((_, overlay)) => Ok(overlay.base),
            None => Ok(self.drawn_source(layer)?.0),
        }
    }

    pub(super) fn write_layer(
        &mut self,
        id: LayerId,
        picture: &Picture,
        linear: bool,
        swaps: &mut Vec<Swap>,
    ) -> Result<u32> {
        match &mut self.state.content {
            Content::Picture {
                picture: current, ..
            } => {
                swaps.extend(Swap::from_pictures(current, picture.clone())?);
                Ok(PLAIN)
            }
            Content::Photoshop {
                document, touched, ..
            } => {
                let mut record = develop::pixel_layer(document, id)?.clone();
                let key = match record.id() {
                    Some(key) => key,
                    None => {
                        let key = arrange::next_id(document);
                        record.set_info(Info::Id(key));
                        key
                    }
                };
                let rewritten_record =
                    develop::rewritten(document, record, picture, linear, Compression::Rle)?;
                let node = layer::node_mut(document, id).ok_or_else(|| no_layer(id))?;
                swaps.push(Swap::Layer {
                    layer: id,
                    record: Box::new(mem::replace(node.layer_mut(), rewritten_record)),
                });
                *touched = true;
                Ok(key)
            }
        }
    }

    pub(super) fn develop_overlaid(
        &mut self,
        id: LayerId,
        recipe: &Recipe,
        action: Action,
    ) -> Result<Option<Recorded>> {
        let Some((key, overlay)) = self.overlay_of(id)? else {
            return Ok(None);
        };
        let developed = overlay.base.develop(recipe)?;
        if developed.dimensions() != overlay.base.dimensions() {
            return Err(Error::LayerReshape);
        }
        let baked = overlay::baked(&developed, &overlay.figures)?;
        let mut swaps = Vec::new();
        self.write_layer(id, &baked, overlay.linear, &mut swaps)?;
        let replaced = picture_bytes(&overlay.base);
        let mut overlays = self.state.overlays.clone();
        overlays.set(
            key,
            Some(Overlay {
                base: developed,
                ..overlay
            }),
        );
        swaps.push(self.state.overlay(overlays, replaced));
        Ok(Some(Recorded {
            action,
            swaps,
            pixels: PixelChange::Content,
        }))
    }

    pub(super) fn refit(&mut self, refit: &Refit<'_>, swaps: &mut Vec<Swap>) -> Result<()> {
        if self.state.overlays.is_empty() {
            return Ok(());
        }
        let mut replaced = 0;
        let mut refitted = Vec::new();
        for (key, overlay) in self.state.overlays.iter() {
            let split = match refit.uprights_text {
                true => 0,
                false => overlay
                    .figures
                    .iter()
                    .rposition(|figure| matches!(figure.form, Form::Text(_)))
                    .map_or(0, |last| last + 1),
            };
            let (pressed, floating) = overlay.figures.split_at(split);
            let flattened = overlay::baked(&overlay.base, pressed)?;
            let base = flattened.develop(&(refit.recipe)(!overlay.base.has_alpha()))?;
            let figures = floating
                .iter()
                .map(|figure| figure.with(figure.form.mapped(refit.map, refit.strokes)))
                .collect();
            replaced += picture_bytes(&overlay.base);
            refitted.push((
                key,
                Overlay {
                    base,
                    figures,
                    linear: overlay.linear,
                },
            ));
        }
        let overlays: Overlays = refitted.into_iter().collect();
        self.rebake(&overlays)?;
        swaps.push(self.state.overlay(overlays, replaced));
        Ok(())
    }

    fn rebake(&mut self, overlays: &Overlays) -> Result<()> {
        match &mut self.state.content {
            Content::Picture { picture, .. } => {
                if let Some(overlay) = overlays.get(PLAIN) {
                    *picture = overlay.baked()?;
                }
            }
            Content::Photoshop {
                document, touched, ..
            } => {
                for (key, overlay) in overlays.iter() {
                    let baked = overlay.baked()?;
                    let Some(record) = layer::record_keyed_mut(&mut document.layers, key)
                        .map(|record| record.clone())
                    else {
                        continue;
                    };
                    let rewritten_record = develop::rewritten(
                        document,
                        record,
                        &baked,
                        overlay.linear,
                        Compression::Rle,
                    )?;
                    if let Some(record) = layer::record_keyed_mut(&mut document.layers, key) {
                        *record = rewritten_record;
                    }
                    *touched = true;
                }
            }
        }
        Ok(())
    }

    pub(super) fn layered_overlays(&self, document: &mut rikka_psd::Document) -> Overlays {
        let Content::Picture { .. } = &self.state.content else {
            return self.state.overlays.clone();
        };
        let Some(overlay) = self.state.overlays.get(PLAIN) else {
            return Overlays::default();
        };
        let key = arrange::next_id(document);
        match document.layers.first_mut() {
            Some(rikka_psd::Node::Layer(record)) => {
                record.set_info(Info::Id(key));
                [(key, overlay.clone())].into_iter().collect()
            }
            _ => Overlays::default(),
        }
    }

    pub(super) fn carried(
        &self,
        mut overlays: Overlays,
        document: &rikka_psd::Document,
        carried: &FigureOrigin,
        after: LayerId,
    ) -> Result<Overlays> {
        let present: HashSet<u32> = document
            .layer_records()
            .iter()
            .filter_map(|record| record.id())
            .collect();
        overlays.retain(|key| present.contains(&key));
        match carried {
            FigureOrigin::Kept => {}
            FigureOrigin::Copied { from } => {
                for (original, copy) in paired_keys(document, *from, after) {
                    if let Some(overlay) = overlays.get(original) {
                        let figures = overlay
                            .figures
                            .iter()
                            .map(|figure| Figure::new(self.figure_ids.next(), figure.form.clone()))
                            .collect();
                        let copied = Overlay {
                            figures,
                            ..overlay.clone()
                        };
                        overlays.set(copy, Some(copied));
                    }
                }
            }
            FigureOrigin::Merged => {
                if let Some(key) = keyed(document, after) {
                    overlays.set(key, None);
                }
            }
            FigureOrigin::Unlocked => {
                if let Some(key) = keyed(document, after)
                    && let Some(overlay) = overlays.get(key).cloned()
                {
                    let rgba = develop::rgba_of(&overlay.base, overlay.linear)?;
                    let base = develop::picture_of(
                        &self.imaging,
                        &rgba,
                        document.depth,
                        overlay.base.profile(),
                    )?;
                    overlays.set(key, Some(Overlay { base, ..overlay }));
                }
            }
        }
        Ok(overlays)
    }
}

pub(super) fn figured_recipe(recipe: &Recipe, figures: &[Figure]) -> Recipe {
    let mut full = recipe.clone();
    for step in figure::recipe(figures).steps() {
        full.push(step.clone());
    }
    full
}

fn checked(form: &Form) -> Result<()> {
    match form {
        Form::Text(lettering) if lettering.text.trim().is_empty() => Err(Error::EmptyText),
        Form::Text(lettering) => Ok(lettering.check()?),
        _ if !form.bounds().width().is_finite() || !form.bounds().height().is_finite() => {
            Err(Error::UnboundedFigure)
        }
        _ => Ok(()),
    }
}

fn missing(id: FigureId) -> Error {
    Error::UnknownFigure(id)
}

fn stacked(figures: &[Figure], moved: &[FigureId], to: StackOrder) -> Vec<Figure> {
    let picked = |figure: &Figure| moved.contains(&figure.id);
    match to {
        StackOrder::Front => figures
            .iter()
            .filter(|figure| !picked(figure))
            .chain(figures.iter().filter(|figure| picked(figure)))
            .cloned()
            .collect(),
        StackOrder::Back => figures
            .iter()
            .filter(|figure| picked(figure))
            .chain(figures.iter().filter(|figure| !picked(figure)))
            .cloned()
            .collect(),
        StackOrder::Forward => {
            let mut order = figures.to_vec();
            for at in (0..order.len().saturating_sub(1)).rev() {
                if picked(&order[at]) && !picked(&order[at + 1]) {
                    order.swap(at, at + 1);
                }
            }
            order
        }
        StackOrder::Backward => {
            let mut order = figures.to_vec();
            for at in 1..order.len() {
                if picked(&order[at]) && !picked(&order[at - 1]) {
                    order.swap(at, at - 1);
                }
            }
            order
        }
    }
}

fn keyed(document: &rikka_psd::Document, id: LayerId) -> Option<u32> {
    match layer::node(document, id)? {
        rikka_psd::Node::Layer(record) => record.id(),
        rikka_psd::Node::Group(_) => None,
    }
}

fn paired_keys(document: &rikka_psd::Document, from: LayerId, to: LayerId) -> Vec<(u32, u32)> {
    let keys = |id: LayerId| {
        let mut keys = Vec::new();
        if let Some(node) = layer::node(document, id) {
            layer::records(&mut node.clone(), &mut |record| keys.push(record.id()));
        }
        keys
    };
    keys(from)
        .into_iter()
        .zip(keys(to))
        .filter_map(|(original, copy)| Some((original?, copy?)))
        .collect()
}
