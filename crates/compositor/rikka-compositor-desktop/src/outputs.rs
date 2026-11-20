use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_shell::{LayerId, LockSurfaceId};
use rikka_compositor_types::OutputId;

use crate::arrange::arrange;
use crate::arrangement::{Arrangement, LayerConfigure};
use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::ids::WindowId;
use crate::layers::Levels;
use crate::output::{Output, OutputState};
use crate::workspace::Workspace;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputRemoved {
    pub windows: Vec<WindowId>,
    pub layers: Vec<LayerId>,
    pub lock_surfaces: Vec<LockSurfaceId>,
}

impl<P> Desktop<P> {
    pub fn add_output(&mut self, id: OutputId) -> Result<Vec<WindowId>> {
        if self.outputs.iter().any(|(output, _)| *output == id) {
            return Err(Error::DuplicateOutput(id));
        }
        let layout = self.layouts.get(id).ok_or(Error::UnknownOutput(id))?;
        self.outputs.push((id, OutputState::new(layout)));
        self.levels.insert(id, Levels::default());
        let strays: Vec<WindowId> = self
            .stack
            .iter()
            .copied()
            .filter(|stray| {
                self.windows
                    .get(*stray)
                    .is_some_and(|window| window.workspace.is_none())
            })
            .collect();
        for stray in &strays {
            if let Some(window) = self.windows.get_mut(*stray) {
                window.workspace = Some(Workspace::new(id, 1));
            }
            self.settle(*stray);
        }
        Ok(strays)
    }

    pub fn remove_output(&mut self, id: OutputId) -> Result<OutputRemoved> {
        let at = self
            .outputs
            .iter()
            .position(|(output, _)| *output == id)
            .ok_or(Error::UnknownOutput(id))?;
        self.outputs.remove(at);
        let refuge = self.outputs.first().map(|(output, _)| *output);
        let windows: Vec<WindowId> = self
            .stack
            .iter()
            .copied()
            .filter(|window| {
                self.windows
                    .get(*window)
                    .and_then(|window| window.workspace)
                    .is_some_and(|workspace| workspace.output() == id)
            })
            .collect();
        for window in &windows {
            if let Some(record) = self.windows.get_mut(*window) {
                record.workspace = record.workspace.and_then(|workspace| {
                    refuge.map(|refuge| Workspace::new(refuge, workspace.number()))
                });
            }
            self.settle(*window);
        }
        let layers: Vec<LayerId> = self
            .layer_order
            .iter()
            .copied()
            .filter(|layer| {
                self.layers
                    .get(*layer)
                    .is_some_and(|layer| layer.output == id)
            })
            .collect();
        for layer in &layers {
            self.remove_layer(*layer);
        }
        self.levels.remove(id);
        let lock_surfaces: Vec<LockSurfaceId> = self
            .lock
            .surfaces
            .iter()
            .filter(|(_, guard)| guard.output == id)
            .map(|(surface, _)| surface)
            .collect();
        for surface in &lock_surfaces {
            self.lock.surfaces.remove(*surface);
        }
        Ok(OutputRemoved {
            windows,
            layers,
            lock_surfaces,
        })
    }

    pub fn output(&self, id: OutputId) -> Option<Output> {
        self.output_record(id)
    }

    pub fn outputs(&self) -> impl Iterator<Item = (OutputId, Output)> + '_ {
        self.outputs
            .iter()
            .filter_map(|(id, state)| Some((*id, state.with(self.layouts.get(*id)?))))
    }

    pub fn output_at(&self, point: Point<f64, Logical>) -> Option<OutputId> {
        self.outputs()
            .find(|(_, output)| output.geometry().to_f64().contains(point))
            .map(|(id, _)| id)
    }

    pub fn arrange(&mut self, id: OutputId) -> Result<Arrangement> {
        let full = self
            .output_record(id)
            .ok_or(Error::UnknownOutput(id))?
            .geometry();
        let states: Vec<_> = self
            .layer_order
            .iter()
            .filter_map(|layer| {
                let record = self.layers.get(*layer)?;
                (record.output == id).then_some((*layer, record.state))
            })
            .collect();
        let arranged = arrange(full, &states);
        let mut arrangement = Arrangement::default();
        for (layer, fit) in arranged.placed {
            let Some(geometry) = fit else {
                self.remove_layer(layer);
                arrangement.closed.push(layer);
                continue;
            };
            let Some(record) = self.layers.get_mut(layer) else {
                continue;
            };
            record.geometry = geometry;
            if record.configured != Some(geometry.size) {
                record.configured = Some(geometry.size);
                arrangement.configures.push(LayerConfigure {
                    layer,
                    size: geometry.size,
                });
            }
        }
        if let Some(record) = self.output_state_mut(id) {
            arrangement.work_area_changed = record.work_area != arranged.work_area;
            record.work_area = arranged.work_area;
        }
        Ok(arrangement)
    }

    pub(crate) fn output_record(&self, id: OutputId) -> Option<Output> {
        let state = self
            .outputs
            .iter()
            .find(|(output, _)| *output == id)
            .map(|(_, state)| *state)?;
        Some(state.with(self.layouts.get(id)?))
    }

    pub(crate) fn output_state_mut(&mut self, id: OutputId) -> Option<&mut OutputState> {
        self.outputs
            .iter_mut()
            .find(|(output, _)| *output == id)
            .map(|(_, output)| output)
    }
}
