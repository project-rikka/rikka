mod adjust;
mod arrange;
mod draw;
mod objects;
mod select;
mod transform;

use paint_assistant::{Change, FloodOrder, LayerRef, Sample};
use paint_document::{Area, Document, Edit, Flood, Layer, LayerId, Source};
use rikka_image_edit::Mask;

use super::layers;
use crate::colors::Colors;
use crate::options::Options;
use crate::tab::Refusal;

const TOLERANCE: f32 = 0.5;

pub(super) struct Defaults {
    pub(super) colors: Colors,
    pub(super) options: Options,
}

pub(super) fn compose(
    document: &mut Document,
    change: Change,
    active_layer: Option<LayerId>,
    defaults: &Defaults,
) -> Result<Vec<String>, Refusal> {
    let mut composer = Composer {
        document,
        active_layer,
        defaults,
        notes: Vec::new(),
    };
    match change {
        Change::Adjust(steps) => composer.adjust(steps)?,
        Change::Draw(drawing) => composer.draw(drawing)?,
        Change::Select(steps) => composer.select(steps)?,
        Change::Layers(steps) => composer.arrange(steps)?,
        Change::Transform(steps) => composer.transform(steps)?,
        Change::Objects(steps) => composer.objects(steps)?,
    }
    Ok(composer.notes)
}

struct Composer<'a> {
    document: &'a mut Document,
    active_layer: Option<LayerId>,
    defaults: &'a Defaults,
    notes: Vec<String>,
}

fn refused(reason: impl Into<String>) -> Refusal {
    Refusal(reason.into())
}

fn stepped<T>(what: &str, at: usize, kind: &str, result: Result<T, Refusal>) -> Result<T, Refusal> {
    result.map_err(|Refusal(reason)| Refusal(format!("{what} {} ({kind}): {reason}", at + 1)))
}

impl Composer<'_> {
    fn apply(&mut self, edit: &Edit) -> Result<(), Refusal> {
        let change = self.document.apply(edit)?;
        if change.layer.is_some() {
            self.active_layer = change.layer;
        }
        Ok(())
    }

    fn size(&self) -> (u32, u32) {
        (self.document.width(), self.document.height())
    }

    fn canvas(&self) -> Area {
        let (width, height) = self.size();
        Area::sized(width, height)
    }

    fn layer(&self, requested: Option<&LayerRef>) -> Result<Layer, Refusal> {
        let all_layers = self.document.layers();
        let resolved = match requested {
            Some(layer_ref) => layers::find(&all_layers, layer_ref).map_err(Refusal)?,
            None => {
                let active_layer = self
                    .active_layer
                    .ok_or_else(|| refused("no layer is chosen; name one with `layer`"))?;
                layers::every_layer(&all_layers)
                    .into_iter()
                    .find(|layer| layer.id == active_layer)
                    .ok_or_else(|| {
                        refused(format!(
                            "the chosen layer is gone; name one with `layer`: {}",
                            layers::listing(layers::every_layer(&all_layers))
                        ))
                    })?
            }
        };
        Ok(resolved.clone())
    }

    fn paintable(&self, requested: Option<&LayerRef>) -> Result<LayerId, Refusal> {
        let layer = self.layer(requested)?;
        layers::paintable(&self.document.layers(), &layer).map_err(Refusal)?;
        Ok(layer.id)
    }

    fn selection_mask(&self) -> Result<Option<Mask>, Refusal> {
        let canvas = self.canvas();
        let selection = self.document.selection().clipped(canvas);
        if selection.is_empty() || selection.encloses(canvas) {
            return Ok(None);
        }
        Ok(Some(selection.mask(canvas.width, canvas.height)?))
    }

    fn flood(
        &self,
        order: &FloodOrder,
        sample: Sample,
        layer: Option<LayerId>,
    ) -> Result<Flood, Refusal> {
        let source = match order.sample.unwrap_or(sample) {
            Sample::Image => Source::Image,
            Sample::Layer => Source::Layer(match layer {
                Some(layer) => layer,
                None => self.layer(None)?.id,
            }),
        };
        let (x, y) = order.at;
        let (width, height) = self.size();
        if x >= width as f32 || y >= height as f32 {
            return Err(refused(format!(
                "({x}, {y}) lies outside the {width}×{height} picture"
            )));
        }
        Ok(Flood::from_point(x, y)
            .tolerance(order.tolerance.unwrap_or(TOLERANCE))
            .mode(order.mode.unwrap_or_default())
            .source(source)
            .anti_aliased(order.antialias.unwrap_or(true)))
    }
}
