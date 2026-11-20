use rikka_compositor_geometry::{Logical, Size};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::zwlr_layer_shell_v1::Layer;
use rikka_compositor_protocols::zwlr_layer_surface_v1::{
    Anchor, Error, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};
use rikka_compositor_surface::SurfaceId;
use rikka_wayland_wire::ClientId;

use crate::lifecycle::Lifecycle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Margins {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerState {
    pub layer: Layer,
    pub anchor: Anchor,
    pub exclusive_zone: i32,
    pub exclusive_edge: Anchor,
    pub margins: Margins,
    pub keyboard_interactivity: KeyboardInteractivity,
    pub size: Size<i32, Logical>,
}

impl LayerState {
    pub(crate) fn new(layer: Layer) -> Self {
        Self {
            layer,
            anchor: Anchor::empty(),
            exclusive_zone: 0,
            exclusive_edge: Anchor::empty(),
            margins: Margins::default(),
            keyboard_interactivity: KeyboardInteractivity::None,
            size: Size::zero(),
        }
    }

    pub(crate) fn violation(&self) -> Option<Error> {
        let unsized_width = self.size.w == 0 && !self.anchor.contains(Anchor::LEFT | Anchor::RIGHT);
        let unsized_height =
            self.size.h == 0 && !self.anchor.contains(Anchor::TOP | Anchor::BOTTOM);
        if unsized_width || unsized_height {
            return Some(Error::InvalidSize);
        }
        (!self.anchor.contains(self.exclusive_edge)).then_some(Error::InvalidExclusiveEdge)
    }
}

#[derive(Debug)]
pub struct LayerSurface {
    pub(crate) resource: ZwlrLayerSurfaceV1,
    pub(crate) surface: SurfaceId,
    pub(crate) output: Option<WlOutput>,
    pub(crate) namespace: String,
    pub(crate) initial: LayerState,
    pub(crate) lifecycle: Lifecycle<Size<i32, Logical>, LayerState>,
    pub(crate) closed: bool,
}

impl LayerSurface {
    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    pub fn client(&self) -> ClientId {
        self.resource.client()
    }

    pub fn version(&self) -> u32 {
        self.resource.version()
    }

    pub fn output(&self) -> Option<&WlOutput> {
        self.output.as_ref()
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn state(&self) -> &LayerState {
        &self.lifecycle.state
    }

    pub fn size(&self) -> Option<Size<i32, Logical>> {
        self.lifecycle.current().copied()
    }

    pub fn awaiting_ack(&self) -> bool {
        self.lifecycle.awaiting_ack()
    }

    pub fn initial_configure_sent(&self) -> bool {
        self.lifecycle.configure_sent()
    }

    pub fn mapped(&self) -> bool {
        self.lifecycle.mapped()
    }

    pub fn closed(&self) -> bool {
        self.closed
    }
}
