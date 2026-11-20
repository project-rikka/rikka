use rikka_compositor_geometry::{Logical, Rect, Size};
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer as Level;
use rikka_compositor_protocols::zwlr_layer_surface_v1::KeyboardInteractivity;
use rikka_compositor_shell::LayerState;
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;

use crate::layer_order::LayerOrder;

#[derive(Debug, Clone)]
pub struct Layer {
    pub(crate) surface: SurfaceId,
    pub(crate) output: OutputId,
    pub(crate) state: LayerState,
    pub(crate) geometry: Rect<i32, Logical>,
    pub(crate) mapped: bool,
    pub(crate) configured: Option<Size<i32, Logical>>,
    pub(crate) order: LayerOrder,
    pub(crate) sequence: u64,
}

impl Layer {
    pub(crate) fn new(
        surface: SurfaceId,
        output: OutputId,
        state: LayerState,
        order: LayerOrder,
        sequence: u64,
    ) -> Self {
        Self {
            surface,
            output,
            state,
            geometry: Rect::zero(),
            mapped: false,
            configured: None,
            order,
            sequence,
        }
    }

    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    pub fn output(&self) -> OutputId {
        self.output
    }

    pub fn state(&self) -> &LayerState {
        &self.state
    }

    pub fn level(&self) -> Level {
        self.state.layer
    }

    pub(crate) fn stacking_key(&self) -> (LayerOrder, u64) {
        (self.order, self.sequence)
    }

    pub fn keyboard_interactivity(&self) -> KeyboardInteractivity {
        self.state.keyboard_interactivity
    }

    pub fn geometry(&self) -> Rect<i32, Logical> {
        self.geometry
    }

    pub fn mapped(&self) -> bool {
        self.mapped
    }
}
