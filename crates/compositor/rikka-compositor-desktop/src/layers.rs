use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::KeyboardInteractivity;
use rikka_compositor_shell::{LayerId, LayerState};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;
use slotmap::SecondaryMap;

use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::layer::Layer;
use crate::layer_order::LayerOrder;

#[derive(Debug, Default)]
pub(crate) struct Levels([Vec<LayerId>; 4]);

impl Levels {
    fn at(&self, level: Level) -> &[LayerId] {
        &self.0[index(level)]
    }

    fn insert(&mut self, id: LayerId, layer: &Layer, layers: &SecondaryMap<LayerId, Layer>) {
        let bucket = &mut self.0[index(layer.level())];
        let at = bucket.partition_point(|stacked| {
            layers
                .get(*stacked)
                .is_some_and(|stacked| stacked.stacking_key() < layer.stacking_key())
        });
        bucket.insert(at, id);
    }

    fn remove(&mut self, id: LayerId) {
        for bucket in &mut self.0 {
            bucket.retain(|stacked| *stacked != id);
        }
    }
}

fn index(level: Level) -> usize {
    match level {
        Level::Background => 0,
        Level::Bottom => 1,
        Level::Top => 2,
        Level::Overlay => 3,
    }
}

impl<P> Desktop<P> {
    pub fn add_layer(
        &mut self,
        id: LayerId,
        surface: SurfaceId,
        output: OutputId,
        state: LayerState,
        order: LayerOrder,
    ) -> Result<()> {
        self.output_record(output)
            .ok_or(Error::UnknownOutput(output))?;
        if self.layers.contains_key(id) {
            return Err(Error::DuplicateLayer(id));
        }
        self.layer_sequence += 1;
        let layer = Layer::new(surface, output, state, order, self.layer_sequence);
        if let Some(levels) = self.levels.get_mut(output) {
            levels.insert(id, &layer, &self.layers);
        }
        self.layers.insert(id, layer);
        self.layer_popups.insert(id, Vec::new());
        self.layer_order.push(id);
        Ok(())
    }

    pub fn set_layer_state(&mut self, id: LayerId, state: LayerState) -> Result<()> {
        let layer = self.layers.get_mut(id).ok_or(Error::UnknownLayer(id))?;
        let moved = layer.state.layer != state.layer;
        layer.state = state;
        if moved && let Some(levels) = self.levels.get_mut(layer.output) {
            levels.remove(id);
            levels.insert(id, &self.layers[id], &self.layers);
        }
        Ok(())
    }

    pub fn set_layer_mapped(&mut self, id: LayerId, mapped: bool) -> Result<()> {
        self.layers
            .get_mut(id)
            .ok_or(Error::UnknownLayer(id))?
            .mapped = mapped;
        Ok(())
    }

    pub fn remove_layer(&mut self, id: LayerId) -> Option<Layer> {
        let layer = self.layers.remove(id)?;
        self.layer_order.retain(|ordered| *ordered != id);
        if let Some(levels) = self.levels.get_mut(layer.output) {
            levels.remove(id);
        }
        if let Some(popups) = self.layer_popups.remove(id) {
            self.drop_popups(popups);
        }
        Some(layer)
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(id)
    }

    pub fn layers_on(
        &self,
        output: OutputId,
    ) -> impl DoubleEndedIterator<Item = (LayerId, &Layer)> + '_ {
        self.layer_order.iter().filter_map(move |id| {
            self.layers
                .get(*id)
                .filter(|layer| layer.output == output)
                .map(|layer| (*id, layer))
        })
    }

    pub fn keyboard_layer(&self, output: OutputId) -> Option<LayerId> {
        [Level::Overlay, Level::Top].into_iter().find_map(|level| {
            self.layers_at(output, level)
                .rev()
                .find(|(_, layer)| {
                    layer.mapped
                        && layer.keyboard_interactivity() == KeyboardInteractivity::Exclusive
                })
                .map(|(id, _)| id)
        })
    }

    pub(crate) fn layers_at(
        &self,
        output: OutputId,
        level: Level,
    ) -> impl DoubleEndedIterator<Item = (LayerId, &Layer)> + '_ {
        self.levels
            .get(output)
            .map_or(&[][..], |levels| levels.at(level))
            .iter()
            .filter_map(move |id| self.layers.get(*id).map(|layer| (*id, layer)))
    }
}
